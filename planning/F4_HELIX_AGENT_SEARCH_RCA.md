# Fix 4 RCA — Helix entry regresses on agent_search clearing

**Date:** 2026-05-19
**Sources:** `crates/rs_cam_core/src/adaptive3d/path.rs`, `WANAKA_ASSESSMENT_2026-05-19.md` (Phase 2 round 3 experiment)

## Symptoms

Switching `Adaptive3dConfig.entry_style` from `Plunge` to `Helix` on a project using `clearing_strategy: agent_search` produced:

- **TP1 Back Rough:** moves 6515 → 14368 (+121 %), normal engagement 59.7 % → 50.8 %, air-cut 34 % → 48.6 %
- **TP6 3D Rough 6:** moves 10845 → 31033 (+186 %), normal 51.1 % → 41.7 %, air-cut 54.8 % → 71.4 %
- **Project:** 0 → 2 rapid collisions, runtime 5221 → 6433 s (+23 %)

## Root cause

In `adaptive3d/path.rs:861-934`, the per-Z-pass entry segment exists in two variants:

1. **`Adaptive3dSegment::Rapid(entry)`** — naïve entry from `safe_z` to `entry.z`
2. **`Adaptive3dSegment::RapidWithFloor { entry, rapid_floor_z }`** — `agent_search` optimization. The clearing function sampled the dexel under the entry XY and tells us "there's nothing solid down to `rapid_floor_z`" — so the planner can rapid through cleared air and only peck the fresh-material descent.

For the `Plunge` entry style on `RapidWithFloor` (lines 867-889):
```rust
let descent_floor = (*rapid_floor_z + RAPID_DESCENT_BUFFER_MM)
    .min(params.safe_z)
    .max(entry.z);
if descent_floor < params.safe_z - 1e-6 {
    tp.rapid_to(P3::new(entry.x, entry.y, descent_floor));
}
emit_peck_plunge(&mut tp, entry, descent_floor, params);
```
Rapids through the cleared air, then pecks only the final descent.

**For `Helix` and `Ramp` (lines 893-918) the `rapid_floor_z` hint is silently ignored:**
```rust
EntryStyle3d::Helix { radius, pitch } => {
    lift_to_safe_z(&mut tp, params.safe_z);
    tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
    let helix_start = P3::new(entry.x, entry.y, params.safe_z);
    crate::dressup::emit_helix(
        &mut tp,
        &helix_start,
        entry,
        radius,
        pitch,
        params.plunge_rate,
    );
}
// Comment in source: "Helix and Ramp entries already self-pace their
// descent; the rapid-floor optimisation doesn't apply"
```

The comment is wrong. There are two distinct consequences:

### Consequence A — Air-cut inflation

The helix descends at `plunge_rate` for the **entire** drop from `safe_z` (typically stock_top + clearance) to `entry.z`. On later Z-passes the descent is large (e.g., from z=25 to z=10 = 15 mm), and >90 % of that descent is through cleared air at controlled feed rate. The sim correctly classifies these moves as in-cut but no-material (air), inflating the air-cut percentage.

### Consequence B — Rapid collisions

The helix coil orbits the entry XY at `radius = helix_radius_factor × D` (default 0.3 × 6 mm = 1.8 mm). At each Z level during descent, the **coil's circumference** sweeps a circle of radius 1.8 mm around the entry XY.

On a multi-Z-pass descent (`safe_z → entry.z` crossing 5–10 already-cleared Z levels), the helix may pass through XY positions that were cleared **at the entry's Z level** but NOT at neighbouring Z levels. The cutter physically intersects uncleared neighbouring stock as it coils down — moving at `plunge_rate` (faster than safe-rapid-into-material), which the sim's collision detector correctly flags as a rapid-into-material event.

Plunge entry doesn't have this problem because it descends straight down on the entry XY, which the agent has explicitly cleared at every Z it has visited.

## Which side is broken

**Side A (helix planner) is broken.** The sim is reporting reality correctly: the helix really does enter uncleared stock at feed rate. The fix has to be in the helix entry path.

## Proposed fix

Honor `rapid_floor_z` in the helix and ramp variants the same way `Plunge` does:

```rust
EntryStyle3d::Helix { radius, pitch } => {
    let descent_floor = (*rapid_floor_z + RAPID_DESCENT_BUFFER_MM)
        .min(params.safe_z)
        .max(entry.z);
    lift_to_safe_z(&mut tp, params.safe_z);
    tp.rapid_to(P3::new(entry.x, entry.y, params.safe_z));
    if descent_floor < params.safe_z - 1e-6 {
        tp.rapid_to(P3::new(entry.x, entry.y, descent_floor));
    }
    let helix_start = P3::new(entry.x, entry.y, descent_floor);
    crate::dressup::emit_helix(
        &mut tp, &helix_start, entry, radius, pitch, params.plunge_rate,
    );
}
```

**Subtlety:** the helix `radius` extends 1.8 mm beyond the entry XY. The `rapid_floor_z` was sampled at the entry XY only. The fix is **necessary but not sufficient** — the helix coil at `descent_floor` may still intersect uncleared neighbouring stock at radius 1.8 mm. A more correct fix would sample the dexel at a circle of `radius` around the entry XY and use the **maximum** of those samples as `helix_safe_floor_z`. But that requires plumbing dexel access into the entry emitter, which is a bigger change.

The minimum-viable fix above will resolve the air-cut inflation completely and will likely resolve the rapid collisions in most cases (because the cleared region for `agent_search` typically extends beyond the helix radius by construction — the agent walks several stepovers before entering the next region). The dexel-sampled correct fix is a follow-up.

## Why this affects `agent_search` specifically

The `RapidWithFloor` segment is emitted only when the clearing function knows the air-floor depth — which is the `agent_search` optimization. With `ContourParallel` clearing the same code path uses naïve `Rapid` segments where helix is fine (no floor info to ignore).

This explains why the current `Plunge` default works on this project (agent_search) but other projects on `ContourParallel` with helix don't show the regression: helix-on-ContourParallel is fine because the entry always starts from `safe_z` regardless.

## Related work

- `planning/AGENTSEARCH_INVESTIGATION_LOG.md` carries other agent_search behaviour notes
- Roadmap B.5 set helix as the default for **2D** Adaptive dressups (`DressupConfig.entry_style`), but does NOT touch `Adaptive3dConfig.entry_style` (which is a separate field — see PRE_OPTIMIZE_DEFAULTS_AUDIT.md)

## Recommendation

Implement the minimum-viable fix as Fix 4b. With this fix in place:

- The current `Plunge` default for `Adaptive3dConfig` could be reconsidered (B.5-style change to `Helix` becomes safer)
- Or leave the default at `Plunge` for the simplicity of "plunge into pre-cleared agent_search air" — fine for wood, sub-optimal for hardness > 1.40 materials where helix would reduce tool stress

**Default decision deferred** until the implementation lands and we can empirically compare helix-with-rapid-floor against plunge on the same Wanaka project.
