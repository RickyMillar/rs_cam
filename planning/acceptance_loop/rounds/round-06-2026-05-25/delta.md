# Round-06 delta — 2026-05-25

**Vs baseline:** round-05 `rounds/round-05-2026-05-25/delta.md`.

**Smoke type:** F-026 fix verification, focused on the three round-05
F-026-evidence cases (AS013, AS015, AS004). Skipped F-024 stability
re-check on AS001-AS006 (F-026 only touches the auto_from_model load
path; non-auto cases cannot have changed by design). Skipped the 11
round-05-deferred cases (will re-run after F-027 + F-028 land).

**Auditor:** autonomous Claude session.

**Implementer commit:** `cad1fcc` (F-026 — auto_from_model load-time
stock bbox re-derivation).

## Headline

**F-026 verified at the load-bbox-fix scope.** On project load with
`stock.auto_from_model = true`, `StockConfig::update_from_bbox` now
fires from `build_session_from_project` (mirroring `add_model`'s
runtime path). On `ux_3d_terrain.toml`: stock.z grew **30 → 57.57**
(model max Z 52.57 + padding 5). On `ux_step_plate_mdf.toml`:
stock.z grew **12 → 15** (model max Z 10 + padding 5).

But **deflection bar didn't move** — the three round-05 misses
remain Exceeds, each from a DIFFERENT root cause exposed once F-026
removed the stale-stock confound:

| Case | Round-05 deflection | Round-06 deflection | Residual finding |
|---|---:|---:|---|
| AS013 adaptive3d | 0.573 Exceeds | 0.576 Exceeds | **F-027** (adaptive3d planner stock-XY mismatch, opened by implementer) |
| AS015 scallop | 0.434 Exceeds | 0.434 Exceeds | **F-027** sibling (finishing doesn't see F-026 stock-growth because the model bbox is unchanged) |
| AS004 face | 0.204 Exceeds (axial 9.14) | **0.243 Exceeds** (axial **11.42**) | **F-028 NEW** — face op z-frame mismatch, *worse* post-F-026 because it scales with stock height |

## F-026 verification: what F-026 fixed and what it didn't

**Fixed:** the load-time bbox staleness. `stock_bbox()` now reflects
the actual loaded model when `auto_from_model = true`. Verified by
the implementer's two acceptance tests:
- `auto_from_model_load_grows_stock_z_to_enclose_terrain_mesh`
- `auto_from_model_load_grows_stock_xy_to_enclose_polygon_model`

Both pass; both fail on master pre-fix.

**Not fixed:** the deflection over-fire that round-05 attributed to
F-026 candidate 2. Round-06 confirms the deflection residual is
**three separate bugs**, not one:

1. **AS013 (3D rough)** — adaptive3d's planner-internal `material_stock`
   is bounded by `mesh.bbox + tool_radius`, but the simulator's
   per-setup dexel grid (post-F-026) is bounded by world stock bbox.
   Cells inside simulator grid but outside planner grid are never
   stamped → final pass scrapes virgin material → axial spike.
   This is F-027. Opened by the implementer during F-026 landing.
2. **AS015 (3D finish, scallop)** — same family as F-027. Scallop
   doesn't see F-026's stock growth (its toolpath is driven by the
   model surface, not stock), so the deflection metric is unchanged
   between round-05 and round-06. Still Exceeds, same root cause as
   F-027.
3. **AS004 (2D face)** — F-028 NEW. Face op emits z_level=-0.5 which
   the simulator interprets as world Z=-0.5 (below stock bottom z=0),
   sitting below the entire dexel grid → every ray "cleared" →
   peak_axial reads ~stock_height. F-026's stock-growth from 12 to
   15 mm made the bug **worse** (axial 9.14 → 11.42), confirming
   it scales with stock height and is independent of F-026's fix
   shape. Filed as F-028 with concrete repro + fix shape.

## AS013 secondary metric changes

F-026 grew the auto_from_model stock for `ux_3d_terrain.toml` from
30 mm to 57.6 mm. adaptive3d's planner now correctly sees the full
workpiece, so the post-fix toolpath is **2.3× larger**:

| Metric | Round-05 (stale stock.z=30) | Round-06 (stock.z=57.6) |
|---|---:|---:|
| Move count | 181,416 | **415,953** |
| Cutting distance | 101,270 mm | 280,058 mm |
| Rapid distance | 71,555 mm | 316,482 mm |
| `rapid_collision_count` | 844 | **2,924** |
| `air_cut_percentage` | 89.2% | 87.3% |
| `deflection.peak_mm` | 0.573 Exceeds | 0.576 Exceeds |
| `power.peak_kw` | 0.151 Within | 0.222 Within |

The collision count went UP, but per-rapid-distance it actually
improved slightly: round-05 = 84 mm / collision, round-06 = 108 mm /
collision. The absolute count rose because the planner now correctly
emits cuts to clear all 52 mm of terrain (previously truncated to 30
mm). This is direction-correct behavior masked by an unnormalized
metric.

**F-017 (rapid collisions)** cannot close on this round — although
the per-rapid rate improved, absolute counts rose because of the
toolpath-size increase. Wait until F-027 lands and re-evaluate.

## Verdict by finding

### Verified (closing this round)

| Finding | Resolution |
|---|---|
| **F-026** | Verified at load-bbox-fix scope (the only thing the fix touches). Stock bbox correctly grows to enclose model. Three residual deflection failures attributed (correctly, this time) to F-027 (AS013/AS015) and F-028 (AS004). |

### Opened this round

| Finding | Trigger |
|---|---|
| **F-028** | Round-06 verification of F-026 on AS004 showed peak_axial scaled with stock height (9.14 → 11.42 when stock grew 12 → 15). Face op z-frame bug independent of F-026 and F-027. |

### Confirmed-but-not-changed

| Finding | Status |
|---|---|
| **F-027** | Opened by F-026 implementer; AS013 + AS015 round-06 numbers match their evidence. Awaits implementer pickup. |
| **F-017** | Cannot close — collision count up not down on AS013 (due to bigger toolpath post-fix). Reframe after F-027 lands. |

### Holding stable (not re-tested this round)

| Finding | Notes |
|---|---|
| F-024 | All 5 origin_z=-12 cases (AS001-AS003, AS005, AS006) skipped this round because F-026's load-path fix doesn't touch the non-auto_from_model path. Will re-verify in round-07 alongside the deferred cases. |
| F-015 | Op preconditions not re-tested. |
| F-023 | Diagnostic asymmetry not re-tested. |

### Acceptance bars status

| Bar | Target | Round-05 | Round-06 | Status |
|---|---:|---|---|---|
| Sim chipload calibration (3D ops) | ≥ 95% | 2/2 (AS013, AS015) | 2/2 stable | stable |
| Sim chipload calibration (2D ops) | ≥ 95% | 5/5 | 1/1 (AS004 re-checked, fired correctly) | stable |
| **Sim deflection calibration** | ≥ 95% | 5/7 Within | **5/7 Within (no change)** | F-026 doesn't move this; needs F-027 + F-028 |
| Optimizer refusal correctness | 100% | not re-tested | not re-tested | stable |

## What's next

Recommend round-07 = F-027 + F-028 implementer pickup, then full
re-sweep:

1. **F-027 implementer**: adaptive3d planner stock-XY widening to
   match simulator dexel grid. Fix shape and acceptance test spec
   already in `findings/F-027-...md`.
2. **F-028 implementer**: face op z_level world-frame emission.
   Fix shape and acceptance test spec in `findings/F-028-...md`.
3. **After both land**: round-07 full sweep AS001-AS018 — re-check
   F-024 stability, verify F-027 + F-028 close the deflection bar
   (target: 7/7 Within on the cases this round tested + add the
   deferred 11), close F-017 if collisions drop.

F-027 and F-028 touch different files (`adaptive3d/` vs likely a
2.5D op planner), so they can be parallelized — different background
agents on disjoint code paths.
</parameter>
</invoke>