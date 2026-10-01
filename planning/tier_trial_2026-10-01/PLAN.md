# Tiered finishing trial on the 350 board — plan and pre-registration (2026-10-01)

Operator request (2026-10-01): "do a large initiative to test different
tiered milling approaches on the 350 model, to nail down efficiency while
retaining a good finish. Right now a R1 bit all over with a 0.03 scallop is
pretty lovely. Which combination of tools/finishes (normally iso scallop)
gets the most material removed in the least time, with good finish
quality? I have found no quantitative way to predict this, so do a trial of
many methods."

Status: plan. This file is written BEFORE any arm runs. The pass rule and
the windows below do not change after a result is seen; a change is a new
dated section that says why.

## Board

- Project: `planning/fixtures/rivmap100/rivmap100_memory_repro.toml` (the
  rivmap100 terrain x3.5, 350 x 350 x 42 mm; stock 380 x 510 x 46, white
  oak). The operator's own `rivmap350.toml` is not in the repo yet (the
  runner is asked to push it to `fixtures-rivmap350`); the top arms are
  re-run on it when it lands.
- Common to every arm: the fixture's Face and 3D Rough, unchanged. The
  rough time is reported once, apart from the finish times.
- Feeds: every op takes the Suggest feeds (vendor LUT, the CLI's
  `--apply-suggest` route), never a typed feed. An arm whose tool gets no
  feed is reported as refused, not run on a guess.

## Tools

- R1.0 tapered ball (tool 6, in the fixture), R2.0 tapered ball (tool 12,
  copied from the wanaka library as in `rivmap350_tiered_finish_step0_preview`).
- H1: a Ø6.35 ball nose, flagged HYPOTHETICAL (not known to be in the
  operator's library); it only shows whether a larger coarse ball pays.

## Arms

Finish chain after the common rough. "Iso" = Scallop with `iso_field = true`.
Tier arms use `plan_multitool_finishing`; `tolerance` is the tier map's
residual tolerance.

| Arm | Chain |
|---|---|
| A1 | R1 iso, h 0.03, whole board (the operator's reference) |
| A2 / A3 / A4 | R1 iso, h 0.02 / 0.05 / 0.08 (the one-tool time-vs-quality curve) |
| A5 / A6 | R2 iso, h 0.03 / 0.02, whole board (the coarse tool alone) |
| T1-T4 | tiers [R2, R1], IsoScallop both, cusp 0.03, tolerance 0.05 / 0.10 / 0.15 / 0.25 |
| T5 | tiers [R2, R1], planner default strategy (UnifiedFinish), cusp 0.03, tolerance 0.15 (the shipped GUI route) |
| T6 / T7 | tiers [H1, R2, R1], IsoScallop, cusp 0.03, tolerance 0.10 / 0.15 |
| T8 | tiers [H1, R1], IsoScallop, cusp 0.03, tolerance 0.15 |
| T9 | T3 with the R2 tier at h 0.05 (cusp set per tier after planning) |
| C1 | R1 Parallel raster, h-equivalent stepover for 0.03 |
| C2 | R1 Scallop, `iso_field = false` (cascade), h 0.03 |
| F1 | R2 iso h 0.08 semi-finish, then R1 iso h 0.03 whole board (does a lighter load let the R1 pass run faster?) |

## Measures

Time (primary efficiency measure): the kinematic cycle time of the finish
ops (simulation with the machine's kinematics, `total_runtime_s` per
toolpath), rapids and links included. Also: cutting / rapid distance,
entry count.

Finish quality (pointwise, `column_deviations`, dev = stock top − model z):

1. Fine windows: 16 windows of 25 x 25 mm on a fixed lattice, centres at
   x, y ∈ {43.75, 131.25, 218.75, 306.25} mm in the model frame (350·(k+½)/4),
   simulated with every toolpath of the arm at a 0.05 mm cell. 0.05 mm is
   about a tenth of the R1 / h 0.03 stepover (2·√(2·1·0.03) = 0.49 mm), so a
   cusp is sampled about ten times. A window sim is exact for its columns: a
   column's top depends only on the moves that pass over it.
2. Whole board at 0.25 mm: gross defects only (missed material, gouges).
   0.25 mm cannot resolve a 0.03 cusp.

Population: columns inside the model silhouette, at least 3 mm in from its
edge. The same population for every arm.

Read per arm: p50 / p90 / p99 of dev; area fraction with dev > 0.06
(2 x the reference cusp); fraction with dev < −0.02 (gouge); min dev; on
the whole board, the area with dev > 0.30.

## Pass rule ("a finish as good as A1") — pre-registered

An arm passes when, against A1 on the same population:

- p99 ≤ A1 p99 + 0.010 mm;
- area fraction dev > 0.06 ≤ A1 + 0.5 percentage points;
- gouge fraction (dev < −0.02) ≤ A1 + 0.05 percentage points;
- whole-board area dev > 0.30 ≤ 1.1 x A1 + 100 mm².

These four margins are a judgement call (lead, 2026-10-01), not a physical
constant. They are listed for the operator to change. The operator's eye on
the residual maps is the final verdict, as in M7 / M8c.

Efficiency read: among passing arms, the lowest finish time wins. Each arm
is also plotted on the A1-A4 curve (time vs p99): an arm below the curve
beats "just change the scallop on R1".

## Instrument

An ignored core test, `crates/rs_cam_core/tests/tier_trial_350.rs`, one arm
per run (env `TIER_TRIAL_ARM`), release build. It writes one JSON per arm
and the residual maps under `planning/tier_trial_2026-10-01/runs/`. Before
any arm is trusted: A1 is run twice (determinism), and one window is
checked at 0.025 mm against 0.05 mm (the cell does not move p99 by more
than 0.005).
