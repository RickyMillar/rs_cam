# Pre-optimize defaults — code audit

**Date:** 2026-05-19
**Purpose:** Map where each pristine-Wanaka defect identified in `WANAKA_ASSESSMENT_2026-05-19.md` lives in the code, before implementing fixes.

## Default-application flow for fresh toolpaths

1. **`OperationConfig::new_default_with_ctx(op_type, &stock_ctx)`**
   (`compute/catalog.rs:890`) — creates static defaults from each `Default` impl in `compute/operation_configs.rs`, then applies stock-aware overrides for depth fields via `apply_stock_defaults(&ctx)` (Roadmap B.1/B.2/B.3).
2. **`compute_feeds_for_op(tool, material, machine, workholding, &op)` then `apply_feeds_result_to_op(&mut op, &result)`** (Roadmap F.5, post-2026-05-12) — overwrites `feed_rate, plunge_rate, stepover, depth_per_pass` from LUT.
3. Other fields (`entry_style`, `slope_from/to`, `region_ordering`, etc.) stay at static defaults.

**Critical implication:** the Wanaka project was saved **before** Roadmap F.5, so its `feed_rate / plunge_rate / stepover / depth_per_pass` values are static `Default` impl values, **not LUT-derived**. New toolpaths created today get LUT values. This re-frames Fix 3 — see below.

## Fix-by-fix root causes

### Fix 1 — adaptive3d stepover too narrow

**Real defect** in `feeds/mod.rs:502-537`. Adaptive3d correctly maps to `FeedsOperationFamily::Adaptive` (`catalog.rs:332`). For a 6 mm flat tool roughing wood: `ae_factor = 0.14` (line 507), giving `ae = 0.14 × 6 = 0.84 mm` — below the `adaptive_woc_factor = 0.2 × D = 1.2 mm` machine-rigidity target.

`machine.rigidity.adaptive_woc_factor` is applied as a **ceiling** at line 536:
```rust
ae_factor = ae_factor.min(machine.rigidity.adaptive_woc_factor);
```
The 0.14 base is below the 0.20 ceiling so the cap never fires.

**Proposed fix:** raise the wood-grade adaptive `ae_base` for flat tools to ~0.20 (matching the machine factor and the empirical Wanaka result). Material-aware — keep 0.14 for metals where smaller stepover is safer.

### Fix 2 — tool-geometry-aware plunge default

**Real defect** in `material.rs:249-258` and `feeds/mod.rs:357-364`. Material-level `plunge_rate_base()` returns one value per material (1000/hardness for solid wood) and the feeds calc just applies machine safety factor. **No tool-geometry awareness anywhere in the plunge path.**

For Generic Hardwood: `1000 × 1.0 × 0.75 = 750 mm/min` plunge — regardless of tool. Matches TP7's 750 exactly.

**Proposed fix:** in `feeds/mod.rs` after line 364, derate plunge for small ball/tapered-ball geometries:
```rust
let plunge_rate = match input.tool_geometry {
    ToolGeometryHint::Ball | ToolGeometryHint::TaperedBall { .. } => {
        let tip_d = match input.tool_geometry {
            ToolGeometryHint::TaperedBall { tip_radius, .. } => tip_radius * 2.0,
            _ => d,
        };
        plunge_rate.min(150.0 * tip_d.max(1.0))  // 150 mm/min per mm of tip diameter
    }
    _ => plunge_rate,
};
```
Lands ≤ 300 for 1 mm TB, ≤ 750 for 5 mm TB, no change for 6 mm flat EM.

### Fix 3 — drop_cutter ball stepover (LUT is already correct)

**Not a code defect.** The LUT correctly returns `ae_factor = 0.03` for `(TaperedBall, Parallel, Finish)` (`feeds/mod.rs:561-563`) → `ae = 0.03 mm` stepover for 1 mm TB drop_cutter finish. Textbook value (~5 µm scallop).

The Wanaka TP7 stepover of 0.30 mm came from `DropCutterConfig::default().stepover = 1.0` (`operation_configs.rs:433`), saved before Roadmap F.5 made the LUT call at toolpath creation. **Today's fresh toolpaths get the correct 0.03 mm.**

This becomes a **Fix 5** case (existing-project re-derivation policy), not a Fix 3 code change.

### Fix 4 — helix entry on agent_search clearing

**Empirical regression**, not a default defect per se.

`Adaptive3dConfig` has its own `entry_style: Adaptive3dEntryStyle` field (`operation_configs.rs:454`), separate from the `DressupConfig.entry_style` that Roadmap B.5 touches. **Roadmap B.5 does NOT touch `Adaptive3dEntryStyle`.** The static default in `Adaptive3dConfig::default()` is `Plunge` (`operation_configs.rs:488`).

So:
- The "Adaptive3d entry should default to Helix per B.5" framing in my session was **incorrect** — B.5 only affects 2D Adaptive's dressup-level entry style.
- The current default of `Plunge` was empirically validated by my session (helix regressed).
- The interesting question — whether helix could be made to work with `agent_search` clearing — is **research-grade RCA work**, not a fix.

**Decision:** Fix 4 reframes as "investigate whether helix should be made viable for adaptive3d at all." If the answer is yes, then a B.5-style default change for Adaptive3d entry style could land. **Out of scope for this batch.**

### Fix 5 — existing-project re-derivation policy

This is the only mechanism that can fix Fix-3-type issues on old projects. The Wanaka project carries pre-F.5 static defaults for `feed/plunge/stepover/depth` and pre-B.1 `min_z = -50.0`. Loading does not re-derive these.

**Two-option decision:** opt-in "freshen defaults" command, or accept-as-saved.

## Implementation plan

Given the audit:

| Fix | Status | Approach |
|---|---|---|
| Fix 1 | **implementable** | bump wood adaptive `ae_base` in `feeds/mod.rs` to 0.20 |
| Fix 2 | **implementable** | tool-geometry plunge derate in `feeds/mod.rs` |
| Fix 3 | **no code change** | LUT already correct; defers to Fix 5 |
| Fix 4 | **research** | RCA, no fix commit |
| Fix 5 | **decision** | one-page policy doc |

Fixes 1 and 2 ship as a single PR; Fix 4 produces an RCA doc; Fix 5 produces a policy doc.
