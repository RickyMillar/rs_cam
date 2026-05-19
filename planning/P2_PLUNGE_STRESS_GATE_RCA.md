# Priority 2 RCA — Plunge-stress warning for small ball/tapered-ball tools

**Date:** 2026-05-19
**Sources:** `crates/rs_cam_core/src/feeds/mod.rs` (Fix 2 cap), `WANAKA_ASSESSMENT_2026-05-19.md` Phase 3

## Symptom

The pristine Wanaka project sims clean for 8 toolpaths (0 rapid collisions, healthy engagement bins), and the diagnostic report says "OK". But the FSWizard cross-check finds three plunge rates that are unsafe:

| TP | Tool | Plunge | FSWizard band | Risk |
|---|---|---|---|---|
| TP4 Rivers TB | 1 mm tapered ball | 400 mm/min | 100–300 | 33% above top |
| TP5 Lakes TB | 1 mm tapered ball | 400 mm/min | 100–300 | 33% above top |
| TP7 3D Finish 6 | 1 mm tapered ball | **750 mm/min** | 100–300 | **150% above top — cutter tip damage on first plunge** |

The sim cannot catch this: its engagement histogram measures cutting moves only, not entries. The dexel + cylinder-volume engagement model has no concept of plunge stress on tapered geometry. So the sim verdict is silent even though the project would chip the cutter on use.

## Root cause

Two-fold:

1. **No sim-side plunge gate exists.** `tool_load::{chipload, power, deflection}` evaluate continuous cutting samples. None of them look at the operation's `plunge_rate` against tool geometry.

2. **Existing toolpaths bypass the LUT cap.** Fix 2 (`feeds/mod.rs:367-386`, commit c5b9f74) caps plunge at `150 × tip_diameter_mm` for `Ball` and `TaperedBall` geometries at LUT-recommendation time. But pre-Fix-2 projects (like Wanaka) carry static-default plunge rates that weren't routed through the LUT calculator. The cap exists in code but doesn't apply to existing data.

So the system has the rule (150 × tip_d cap) but doesn't enforce it on saved toolpaths.

## Fix

Lift the cap rule into a reusable validator and run it as a project-level diagnostic. Two pieces:

1. **`tool_load::plunge_stress` module** — exposes `safe_plunge_cap_mm_min(geometry: ToolGeometryHint) -> Option<f64>` and `check_plunge_stress(geometry, plunge_rate) -> Option<PlungeStressWarning>`. The cap matches Fix 2 exactly: `150 × tip_diameter_mm`, where tip_diameter is `d` for ball and `(tip_radius × 2).max(0.5)` for tapered-ball. Flat/Bull/VBit return `None` (no cap).

2. **Wire into `Session::diagnostics()`** — like P1's air-cut offenders, build a list of plunge-stress offenders by scanning enabled toolpaths and comparing their operation `plunge_rate` against the cap. Surface in the verdict naming the TP(s) and the rates.

## Why a verdict-level diagnostic, not a `ToolpathLoadVerdict` field

Adding a fourth `PlungeStressVerdict` field to `ToolpathLoadVerdict` would touch ~15 test sites and break serialization compatibility with the MCP wire format. The plunge cap is simpler than the existing gates (no sim trace needed, no LUT lookup, no per-sample reasoning) so it fits naturally at the diagnostics layer alongside the air-cut warnings (P1). The export gate intentionally doesn't block on plunge — this is a Phase 3-style safety nudge surfaced to the user, not a hard refusal.

## Tests

Negative cases (silent — should NOT warn):
- 6 mm flat end mill at 750 mm/min plunge → silent (flat has no cap)
- 6 mm ball at 750 mm/min plunge → silent (750 ≤ 150×6 = 900)
- 1 mm tapered ball at 150 mm/min → silent (at cap)

Positive cases (signal — SHOULD warn):
- 1 mm tapered ball at 750 mm/min → warn (cap = 150 × 1 = 150)
- 1 mm tapered ball at 400 mm/min → warn (cap = 150 × 1 = 150)
- 2 mm ball at 400 mm/min → warn (cap = 150 × 2 = 300)

## Cap rationale

`150 × tip_diameter` is the same number Fix 2 picked from FSWizard/GWizard published ranges for small ball/tapered-ball in wood. The cap unit (mm/min per mm of tip diameter) is a heuristic — keeps the cap proportional to tip cross-section, which is the load-bearing area on a plunge. Larger tip → more flute tip surface absorbing the axial force.
