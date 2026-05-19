# Priority 5 RCA — Load-time validator for stale defaults (Option C)

**Date:** 2026-05-19
**Sources:** `planning/F5_FRESH_DEFAULTS_POLICY.md` (design doc), `WANAKA_ASSESSMENT_2026-05-19.md`

## Background

`planning/F5_FRESH_DEFAULTS_POLICY.md` already documented the decision: don't re-derive defaults on load (Option A), don't accept-as-saved with no warning (Option B), but **flag specific known-stale patterns at load time** with per-rule auto-fix closures (Option C). P5 ships the initial implementation.

The Wanaka audit surfaced four stale-default symptoms. Three of those have specific detection rules that ship in this batch:

1. **`drop_cutter_min_z_pre_b1`** — `min_z <= -49.999` on a DropCutter config. Pre-Roadmap-B.1 default was `-50.0`.
2. **`tapered_ball_plunge_pre_fix2`** — ball / tapered-ball tool with plunge rate above the `150 × tip_diameter_mm` cap. Pre-commit-c5b9f74 plunge defaults weren't tool-geometry-aware.
3. **`wood_adaptive_stepover_pre_fix1`** — wood-class material with flat tool on adaptive op, stepover below `0.15 × tool diameter`. Pre-commit-c5b9f74 wood adaptive used `ae_factor = 0.14` instead of `0.20`.

## Fix

New module `crates/rs_cam_core/src/compute/validate.rs`:

```rust
pub struct StaleDefault {
    pub rule_id: &'static str,
    pub toolpath_id: usize,
    pub toolpath_name: String,
    pub title: String,    // short headline
    pub detail: String,   // 1-2 sentence explanation with auto-fix preview
    pub auto_fix: Box<dyn FnOnce(&mut ProjectSession) + Send>,
}

pub fn validate_stale_defaults(session: &ProjectSession) -> Vec<StaleDefault>;
```

Each rule is a function `(session, &tc, &tool, &material) -> Option<StaleDefault>`. The auto-fix closures take `&mut ProjectSession` and apply the post-improvement default by calling existing setters. The auto-fix is conservative — it sets the **specific** field to the **specific** new default value rather than re-running the whole LUT calc, so no other params change.

UI surfacing (a load-warning panel) is a follow-up; P5 just exposes the rule engine + library so MCP and CLI can consume it now.

## Rule library — initial 3 rules

### `drop_cutter_min_z_pre_b1`

```rust
detect: |op, _, _, _| matches!(op, OperationConfig::DropCutter(c) if c.min_z <= -49.999)
auto_fix: set DropCutter::min_z to stock_bottom_z
```

### `tapered_ball_plunge_pre_fix2`

```rust
detect: |op, _, tool, _| {
    let hint = tool.to_geometry_hint();
    let cap = safe_plunge_cap_mm_min(hint, tool.diameter())?;
    op.plunge_rate() > cap
}
auto_fix: clamp plunge_rate to the cap
```

This rule overlaps with P2's runtime warning. The difference:
- P2 fires in `diagnostics()` after sim runs — surfaced as a "WARNING" in the verdict.
- P5 fires at project load — actionable; the user can apply the auto-fix immediately.

### `wood_adaptive_stepover_pre_fix1`

```rust
detect: |op, mat, tool, _| {
    is_wood_class(mat)
        && matches!(tool.to_geometry_hint(), ToolGeometryHint::Flat)
        && matches!(op, OperationConfig::Adaptive(_) | OperationConfig::Adaptive3d(_))
        && op.as_params().stepover().unwrap_or(0.0) < 0.15 * tool.diameter()
}
auto_fix: set stepover to `machine.rigidity.adaptive_woc_factor × tool.diameter()`
```

`0.15 × D` is the midpoint between the old `0.14` and new `0.20` factor — clearly old defaults sit below, new ones sit above.

## Tests

Per-rule tests:
- `drop_cutter_min_z_pre_b1` fires on `min_z = -50.0`, not on `min_z = -20.0`
- `tapered_ball_plunge_pre_fix2` fires on 1 mm TB at 750 mm/min, not on 1 mm TB at 100, not on 6 mm flat at 750
- `wood_adaptive_stepover_pre_fix1` fires on wood+flat+Adaptive3d at stepover 0.7 mm (6 mm tool); silent on metal, silent on ball tool, silent at 1.2 mm stepover

Integration:
- A session with all three defects produces all three offenders
- Auto-fix on each one moves the value into the new band
- A clean session produces zero offenders

## Out of scope

- GUI surface for the load-warning panel (separate task)
- Adding rules per future B-roadmap entry (ongoing convention per F5 doc)
