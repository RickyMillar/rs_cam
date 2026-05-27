# Bench batch — 2026-05-27 (F-038b + F-039 + F-040 validation)

## Goal

Confirm three workstream features on real Shapeoko XXL hardware:

- **F-038b** — adaptive3d keep-tool-down link (sim predicts −12% Back Rough runtime, less rapid travel)
- **F-039** — constrained-max feed modulation (sim says ConstrainedMax @ 1.0 ≈ BandMid on wanaka; aggressiveness 1.3 trims 17%)
- **F-040** — lead-in / lead-out feed-rate breakout (audible F500 entry / F4500 exit on the small profile fixture)

All six G-code files passed Docker CAMotics 1.2.0 GRBL-parse validation. The arc-fitter fix (commit `2db19c2`) is included, so the wanaka Back Rough should NOT throw GRBL error 33 like it did pre-fix.

## Setup (same as 2026-05-26 bench)

| Item | Value |
|---|---|
| Machine | Shapeoko XXL with Ricky-tuned `$$` (X/Y accel 500 mm/s², Z accel 270 mm/s², X/Y feed 10000 mm/min, Z feed 1000 mm/min) |
| Tool (Back Rough) | End Mill 6mm, 2-flute, hardwood |
| Tool (variant 6 profile) | Same End Mill 6mm |
| Material | Wanaka hardwood blank (same stock as 2026-05-26) |
| Spindle | 18000 RPM, M3 |
| Stock zero | Match wanaka project — top of stock at Z=0 |

## Run order

Run **dry first** for each variant (Z up several mm) to confirm motion / sound before committing to a real cut.

| # | File | Predicted | Notes / What to listen for |
|---|---|---|---|
| 1 | `v1_back_rough_unmodulated.nc` | ~7m 39s | Baseline. Compare wall-clock vs sim. Listen for keep-down links (no full retract between adjacent cuts on flat shelves). |
| 2 | `v2_back_rough_bandmid.nc` | ~14m 16s | F-036 BandMid (pre-F-039 default). Anchors your prior 20:24 measurement under the new shorter toolpath. |
| 3 | `v3_back_rough_cmax_1p0.nc` | ~14m 08s | F-039 ConstrainedMax default. Sim says ~identical to BandMid because binding constraint isn't chipload-max here. Confirm. |
| 4 | `v4_back_rough_cmax_1p3.nc` | ~11m 43s | F-039 aggressiveness 1.3 — pushes 30% past binding. Sim predicts -17% vs v3. Watch for any sound of stress (force the run if tool sounds bad — write down what %, what move type). |
| 5 | `v5_back_rough_cmax_1p0_no_staydown.nc` | ~16m 03s | F-038b stay-down OFF. Should run +14% longer than v3 (sim says +115s ≈ +1m 55s). Audibly more rapids / retracts between regions. |
| 6 | `v6_lead_in_out_profile.nc` | ~33s | F-040 fixture. Star outline profile on hardwood, 4mm depth, 2 passes. **Listen for the feed change** at entry/exit: F500 (slow) on the quarter-circle lead-in, then F1800 cut, then F4500 (fast) on lead-out. |

## What to record per variant

| Field | How to capture |
|---|---|
| Wall-clock start → end | Stopwatch (phone is fine) |
| GRBL errors | gSender or controller log — should be zero |
| Subjective tool sound | "smooth / chatter / scream" — one word + any unusual notes |
| Surface finish (variants 1-5) | Look at the Back Rough wall — any new witness marks vs prior baseline? Photo if obvious |
| Surface finish (variant 6) | Compare the star's lead-in tangent vs the cut edge — softer entry mark on this one? |

## Comparisons we care about

After the run, compare:

1. **v1 vs prior bench (2026-05-26 unmod = 13:47):** sim predicts 7m 39s now (shorter toolpath post-F-038/F-038b). If wall-clock is far from 7m 39s, F-034 calibration drifted — `MachineKinematics::shapeoko_xxl_ricky_tuned` may need re-tune. Record actual time → I'll widen / re-tune.
2. **v3 vs v2 (CMax @ 1.0 vs BandMid):** sim predicts ~equal. If different, the binding-constraint distribution differs from expectation — F-039b sub-finding (binding distribution dump).
3. **v3 vs v5 (stay-down on vs off):** sim predicts +14% with stay-down off. If on-bench delta is far from that, F-038b's heightfield-sampling parameters need tuning.
4. **v3 vs v4 (aggressiveness 1.0 vs 1.3):** sim predicts -17%. If yes, the aggressiveness knob is real and we should expose it more prominently in the UI.
5. **v6 lead-in/out feed:** audible at 500 / 1800 / 4500 transitions. If audible AND surface mark looks cleaner than a default lead-in, F-040 is worth promoting in UI defaults.

## Pre-flight (already done)

- Docker CAMotics 1.2.0 GRBL-parse: all 6 files clean.
- Arc-fitter (commit `2db19c2`) included — wanaka Back Rough arcs all pass `$12=0.010` tolerance.
- F-037 smoke baseline: no regressions in current `master` (commit `77c1101`).
- **`nc-time` cross-check** — second independent time estimate via parsing the .nc and replaying through F-034:

  ```
  cargo run -p rs_cam_cli -- nc-time planning/feed_modulation_calibration/bench_batch_2026-05-27/v*.nc
  ```

  Predicted times (with stock kinematics: 350 mm/s² accel, 4000 mm/min max feed, 10000 mm/min rapid):

  | File | nc-time | project --summary | Note |
  |---|---|---|---|
  | v1 | 6m 45s | 7m 39s | -12% (setup overhead delta) |
  | v2 | 13m 35s | 14m 16s | -5% |
  | v3 | 13m 27s | 14m 08s | -5% |
  | v4 | 11m 01s | 11m 43s | -6% |
  | v5 | 15m 14s | 16m 03s | -5% |
  | v6 | 0m 33s | 0m 33s | match |

  **Relative deltas between variants agree within 1-2 % between the two estimators** — that's the validation: the emitter didn't drop / add moves between IR and .nc, and the modulator's effect is consistent across both. Absolute offset is M3 spinup / dwell / initial rapid-to-stock the project pipeline tracks that the bare .nc replay skips.

## Reset between runs

- Pause spindle (M5), retract Z to safe, no need to re-home unless the run errored mid-cut.
- Same stock zero across all 5 Back Rough runs.
- Variant 6 needs a fresh stock area (profile cuts a star outline ~10cm × 10cm).

## Commits behind this batch

| Feature | Commit |
|---|---|
| F-040 lead-in/out | `77c1101` |
| F-039 constrained-max | `c1a4932` |
| F-038b keep-tool-down | `86d7da8` |
| F-038 entry coalescing | `2860896` |
| F-034 kinematics + F-036c calibration | `a797909` |
| arcfit chord bisector | `2db19c2` |
| F-037 smoke baseline | `bca5e20` |

## After the bench

Bring measurements back. I'll:
- Compare wall-clock vs sim predictions.
- Re-anchor F-034 / F-036c calibration test tolerances if numbers shifted.
- File F-039b (binding-constraint distribution) if v3 / v2 differ on wood.
- Promote F-040 to a UI default if the lead-in surface mark is visibly cleaner.
