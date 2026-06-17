# Strategy advisor — suggest the operation, don't make the user pick

**Status:** DESIGN (concept; no code)
**Date:** 2026-06-17
**Depends on:** KC_MILLING_CALIBRATION_2026-06-17 (real loads) + a machine
acceleration model (new, see §4).

## Goal

`material + tool + machine + region` in → **suggested strategy + suggested
feeds/params** out, with the tool-limited / machine-limited "regime" surfaced
only as the human-readable *why*, never as something the user must reason about.

## Core principle (revised — the speed insight)

The naïve rule "tool-limited ⇒ adaptive/spiral" is **wrong on low-acceleration
machines.** The classic argument assumes the machine can change direction
instantly; it can't. The honest objective is:

> **minimise wall-clock T, subject to peak tool load ≤ limit (and power ≤ limit).**

For each strategy, `T ≈ path_length / effective_feed`:

- **Parallel, backed off to the load limit:** more path (spiky load → average
  below peak → wasted capacity on straights → more passes) **but effective feed
  ≈ commanded** (long straights, machine holds speed).
- **Spiral at the load limit:** less path (constant load, no wasted capacity)
  **but effective feed << commanded** (machine decelerates into every loop).

Spiral wins iff `path_spiral/path_parallel < feed_spiral/feed_parallel`, and
that ratio is set by **acceleration**:

| Machine accel | feed_spiral vs feed_parallel | Winner at the load limit |
|---|---|---|
| High (rigid VMC) | ≈ equal | **Spiral** (less path) |
| Low (belt router) | spiral much slower | **Parallel backed-off** |

Measured on wanaka: parallel 446 s vs spiral 828 s (1.85×) at equal nominal
stepover — a gap large enough that parallel can absorb heavy backing-off and
still win on a Shapeoko-class machine. **So "tool-limited" does not select
spiral; "tool-limited × low accel" actively de-selects it.**

## Genuinely good reasons to pick spiral (the exception list)

1. Rigid / high-accel machine — carries feed through the loops.
2. Deep narrow features parallel can't clear without full-width slotting or
   gouging — trochoidal entry is the only safe option.
3. Tool-life-critical / production runs — constant load = even wear, no fatigue
   cycling, even when wall-clock ties.
4. Max-MRR in hard material — wasted-capacity loss on the straights is large.

For a light wood router, spiral is the exception, not the default.

## Pipeline

```
material → Kc (milling-calibrated)        ┐
tool geometry → deflection model          ├─→ feasible envelope (DOC/WOC/feed
machine → power + rigidity + ACCEL        ┘    where deflection & power ≤ limits)
                                                          │
       ┌──────────────────────────────────────────────────┘
       ▼
  for each candidate strategy:
     params* = argmax MRR s.t. peak load ≤ limit      (constrained optimise — exists in Suggest)
     T_hat   = estimate wall-clock(params*, accel)    (NEW: accel-aware time estimate)
  pick argmin T_hat → suggested strategy + params*
  regime label = which constraint bound params*       (explanation only)
```

## §4 — Machine acceleration model — ALREADY EXISTS (F-034)

Correction from the original draft: the accel model is **not** missing. F-034
shipped `MachineKinematics { acceleration_mm_s2, jerk_mm_s3,
max_junction_velocity_mm_min }` **and** a trapezoidal accel-aware integrator
`machine_kinematics::compute_cycle_time(&Toolpath, &kinematics, max_feed,
rapid_feed) -> seconds` (naive-sum vs accel-aware differ 30–50 %). The user's
own machine is even **calibrated**: `MachineKinematics::shapeoko_xxl_ricky_tuned()`
= blended **350 mm/s²**, fit against measured wanaka Back Rough wall-clock 827 s
(matches the 828 s spiral run this session).

The only gap was *access*: presets ship `kinematics: None` (the F-034 flag that
keeps live-sim `total_runtime_s` byte-identical), so the advisor needs a model
even when unconfigured. Added `MachineProfile::effective_kinematics()` →
explicit kinematics if set, else `generic_wood_router()` (200 mm/s²) default.
**Done + compiles** — the advisor now gets accel from any profile without
touching live-sim behaviour.

So the advisor's wall-clock estimate is simply
`compute_cycle_time(candidate_toolpath, &machine.effective_kinematics(), …)` —
no new physics. The same integrator later powers the orthogonal "smoothness /
violence" knob.

## Strategy mapping (after the optimisation, for explanation)

| Binding limit at params* | + machine accel | Suggest | Why (shown to user) |
|---|---|---|---|
| Tool (deflection) | high | Spiral | "tool is the limit; your machine carries feed through the constant-load path" |
| Tool (deflection) | low | **Parallel (backed off)** | "tool is the limit, but backing off the conventional path is faster than the spiral's decel" |
| Machine (power/feed) | any | Parallel | "machine is the limit; constant-engagement buys nothing" |
| Neither (light cut) | any | Parallel (calmer) | "neither is stressed; the calmer motion suits the machine" |
| Geometry forces it (deep narrow) | any | Spiral | "only safe way to clear this feature without slotting" |

## Orthogonal axis — smoothness / "violence"

Even within a chosen strategy, a low-accel machine wants gentler cornering
(`min_cutting_radius`, feed-ramp). Driven by the *same* accel model. Keep it a
separate knob/auto-derivation; it tunes *how* a strategy runs, not *which* one.

## Guardrails

- **Suggest, never force.** Show recommendation + one-line reason; user accepts /
  overrides. Finish quality, fixturing, operator habit aren't in the model.
- **Per-operation, not per-machine** — regime + winner recomputed per region.
- **Confidence honest** — loads are literature-anchored, not bench-validated; the
  accel model is nominal. Label suggestions "verify on a test cut," not "optimal."
- **Don't over-trust the time estimate** — `T_hat` is a model; surface it as a
  comparison ("~1.8× faster"), not a promise of seconds.

## Build order

1. Add `max_accel_mm_s2` (+ optional junction deviation) to `MachineProfile`;
   populate the shipped presets (Shapeoko low, VMC high).
2. `effective_feed` / wall-clock estimator (accel-aware) — reusable for the
   smoothness knob too.
3. Strategy advisor: run the existing Suggest params-optimise per candidate
   strategy, estimate `T_hat`, pick min; emit strategy + params + regime reason.
4. Surface as an advisory in the op UI (accept / override), not an auto-apply.
5. Sentry: on a Shapeoko-class profile + wood + the wanaka rough, advisor picks
   Parallel; flip the profile to a high-accel preset and it picks Spiral — pins
   the accel-dependence so it can't regress to a naïve regime lookup.
```
