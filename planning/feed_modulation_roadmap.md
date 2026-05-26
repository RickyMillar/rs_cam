# Feed Modulation Workstream — Roadmap

**Status:** planning (2026-05-26)
**Predecessor:** acceptance loop closed at 7/7 deflection bar (round-10)
**Successor:** TBD

## Why this exists

The acceptance loop calibrated the simulation's verdicts — when it says deflection Within / chipload Within / 0 rapid collisions, those numbers reflect physical reality. But the loop did **not** address how realistic the cycle-time prediction and feed-modeling are.

Three concrete gaps surfaced during round-10 wanaka verification:

1. **Cycle time prediction is `distance / feed`** — ignores machine acceleration / jerk. On corner-heavy 3D rough toolpaths, real-world cycle time can be 30-50% longer than predicted.
2. **The gates assume commanded feed = achieved feed** — but on a Shapeoko-class hobby machine, the controller's planner lookahead decelerates through corners; the cutter never reaches the commanded feed in tight geometry. The chipload gate can say "Within" while the actual chipload is below band because feed never lands.
3. **No per-segment feed modulation** — every toolpath emits a single constant `feed_rate`. Industrial CAM (Fusion HSM, Mastercam Dynamic) modulates feed per segment to maintain constant chipload: slower through corners, faster in straight sections. Result: 30-50% cycle-time reduction at constant-or-better tool load. rs_cam doesn't do this.

The workstream resolves these three gaps in increasing scope, plus hardens the regression net so the existing acceptance-loop calibration stays intact.

## Sequencing — four findings

```
F-034 (cycle time estimator)        ──┐
       ↓                                ├──> F-036 (per-segment modulation)
F-035 (predicted feed in gates)     ──┘

F-037 (regression hardening)        ── parallel; lands first, gates the others
```

**F-037 lands first** because it's the safety net for everything that follows. Without F-037, an F-035 regression could land silently and we'd discover it the next time an auditor runs smoke manually.

**F-034 and F-035 are sequential** because F-035 consumes the kinematics model F-034 introduces.

**F-036 depends on F-034 + F-035** because per-segment modulation needs the kinematics model to compute corner-decel-aware modulation curves AND needs the predicted-feed plumbing in the gates to verify the modulated path's chipload stays in band.

## Feature-flag discipline (the core principle)

**Every change in this workstream lands behind a feature flag, default off.**

The loop's `_f0{24,26,27,28,31}.rs` acceptance tests + the smoke suite ARE the regression net. They must continue passing byte-identically with all new flags OFF.

Each finding's acceptance test asserts behavior under BOTH flag states:
- Flag OFF: byte-identical to pre-finding HEAD (no regression)
- Flag ON: new behavior asserted with tight bands

When a flag is mature (multi-round real-world verified), it can flip default. The old path stays as a fallback for one release cycle before deletion.

## Estimated total effort

| Finding | Effort | Sequencing |
|---|:-:|---|
| F-037 (regression hardening) | M | First; parallel with F-034 work |
| F-034 (cycle time estimator) | S-M | After F-037 |
| F-035 (predicted feed in gates) | M | After F-034 |
| F-036 (per-segment modulation) | L-XL | After F-034 + F-035 |
| **Total** | **~4-6 weeks focused work** | |

Roughly: F-037 + F-034 in week 1-2, F-035 in week 3-4, F-036 over 4-6 weeks of its own.

## How regression is guaranteed

Layered:

1. **Cargo acceptance tests** (`_f0{24,26,27,28,31}.rs`) — per-fix invariants. Must pass byte-identical on every PR.
2. **Per-finding two-state tests** (`_f0{34,35,36}.rs`) — assert flag-off byte-identical, flag-on new behavior.
3. **Smoke baseline** (`baselines/2026-05-26.csv`, captured in F-037) — full 18-case + wanaka verdict snapshot. Future PRs diff against this.
4. **CI gate** (F-037 part 3) — nightly job runs smoke against baseline, fails on any verdict regression.
5. **Auditor smoke** (loop machinery) — manual deep-dive when the CI gate or cargo tests fire.

The first PR after the loop closed (F-037) captures today's baseline. Every subsequent PR has a clear "no regression vs that baseline" criterion.

## Open questions for the implementer sessions

These don't need answers now — they get answered during F-034 / F-035 work:

1. **Where does `MachineKinematics` live?** Probably extending the existing `Machine` struct in `rs_cam_core::machine`. Fields: `acceleration_mm_s2: f64`, `jerk_mm_s3: f64`. Reasonable Shapeoko XXL defaults: accel 500 mm/s², jerk 5000 mm/s³ (after the loop closes, may need calibration measurement to refine).
2. **Cycle time integrator algorithm.** Standard trapezoidal velocity profile with jerk limit. Walk each `LinearMove` / `ArcMove`, compute time-to-accelerate + cruise + decel for each. Sum across the toolpath. Subtract `T_naive = distance / feed` from `T_kinematic` to compute the "lost time" surface — this is the corner-decel signal.
3. **Predicted feed semantics.** Per-sample? Per-move? Per-segment? Probably per-move (`LinearMove`, `ArcMove`) computed once at simulation time. Cache on the move; gates read it.
4. **Per-segment feed modulation algorithm.** Industry standard: target constant chipload. For each segment, predicted achievable engagement × chipload target → required feed. Cap at machine's max feed; floor at machine's min feed (or chipload-minimum-from-LUT).
5. **G-code emission of per-segment feed.** Easiest: emit `F<rate>` words on every move whose feed differs from the prior. Most GRBL controllers handle this fine; check for Shapeoko-specific quirks.
6. **Calibration data for cycle time.** Need at least one real-machine cycle-time measurement (e.g. run wanaka's Back Rough on actual Shapeoko, record wall-clock) to validate F-034's integrator against ground truth.

## Out of scope (don't bundle these in)

- **Dynamic RPM modulation** — practically nobody does this (spindle inertia >> feed-modulation timescale). Not in this workstream.
- **HSM/trochoidal toolpaths** — these are toolpath generators, not feed modulators. Separate workstream.
- **Optimizer integration of feed modulation** — the existing optimizer's Stage 1/2 candidate generation could host this, but bundling is too much for F-036. Leave optimizer alone for now.
- **Multi-axis (4/5-axis) considerations** — rs_cam is 3-axis only.

## Per-finding files

- [F-034](acceptance_loop/findings/F-034-machine-kinematics-cycle-time.md) — acceleration-aware cycle time estimator
- [F-035](acceptance_loop/findings/F-035-predicted-feed-in-gates.md) — predicted-effective-feed in gate evaluation
- [F-036](acceptance_loop/findings/F-036-per-segment-feed-modulation.md) — per-segment adaptive feed modulation
- [F-037](acceptance_loop/findings/F-037-smoke-baseline-and-regression-net.md) — smoke baseline + wanaka regression + CI gate

## Per-finding handoff prompts (drop into fresh sessions)

- [F-034 brief](acceptance_loop/handoff_prompts/F-034-cycle-time-brief.md)
- [F-035 brief](acceptance_loop/handoff_prompts/F-035-predicted-feed-brief.md)
- [F-036 brief](acceptance_loop/handoff_prompts/F-036-feed-modulation-brief.md)
- [F-037 brief](acceptance_loop/handoff_prompts/F-037-regression-net-brief.md)

## Workstream-level success criteria

When this workstream closes:

1. `total_runtime_s` matches measured wall-clock on a Shapeoko XXL reference toolpath within ±10%
2. Predicted-feed gate flag mature; corner decel correctly modeled; chipload Within verdicts reflect the achieved feed not the commanded one
3. Per-segment modulation flag mature; opt-in gives 30-50% cycle time reduction on rough toolpaths at constant-or-better tool load
4. Smoke baseline + CI gate established; any future regression in the deflection bar caught pre-merge
5. The acceptance loop's 7/7 deflection bar still holds (wanaka + all 18 smoke cases unchanged)
</parameter>
</invoke>