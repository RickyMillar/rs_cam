# wanaka200 overnight cycle-time tuning — 2026-08-23 (operator asleep)

Goal (operator, 2026-08-22 23:xx): keep cut quality, get the 13.2 h predicted
cycle time "well down"; coarser finish suggested, unified-finish meld on the
table. Method: paired A/B, ONE variable per iteration, sim at 0.15 mm
(0.1 OOMs this board — two GUI deaths on record), quality bars = 0 collisions,
gates Within, crosses-standing not worse, scallop-height math stated per change.
The operator's verified exports (`wanaka200_run_*.nc`) are NOT touched; the
tuned candidate ships as separate files.

Baseline params (from wanaka200.toml as committed 90418330):
- Back Rough  (idx 1): adaptive3d contour_parallel, stepover 1.2, DPP 4.2,
  F750 (mod +30%), mill_shallow OFF, leave axial 4.0 (terrain follows later)
- Front Rough (idx 5): same, stepover 1.2, DPP 4.2, F750 (mod +127%),
  mill_shallow OFF, detect_flat OFF, leave 0.5/0.3
- 3D Finish   (idx 6): drop_cutter R1.5, stepover 0.3 (scallop 7.5 µm!),
  F1260 (mod −38%), slope 0–90
- Pencil      (idx 7): R0.5, rest_depth, ref Ø3, F1851, air 52%

## Scoreboard

| run | change (one var) | total s | Δ vs base | finish s | roughs s | collisions | notes |
|---|---|---|---|---|---|---|---|
| E0 | baseline (as-committed) | 47412 (13.2h) | — | 12143 | 11133 | 0 | gates 8/8 Within; PENCIL = 23306s (49%!), of which 21366s ENTRY intent — fed descents at plunge 135 |
| E1 | pencil hookup_distance 5→15 + plunge 135→150 (cap: tapered_ball_plunge 150 guard REFUSED 400) | 31282 (8.7h) | **−34%** | 12143 | 11133 | 0 | pencil 23306→7175s; entry 21366→5597s; valley CUTTING intact (300→292s); crosses-standing 19.1%→15.8%; links +232s only |
| E2 | finish stepover 0.3→0.6 (scallop 7.5→30 µm) | 29365 (8.2h) | −38% | 6027 | 11133 | 0 | finish −6116s BUT pencil +4200s (rest detector chases coarser residuals at its 0.05 floor); mid-seq 44 rapid collisions = stale rest chain, cleared by pencil regen — the E-loop rule held |
| E3 | pencil min_valley_depth 0.05→0.10 | 22921 (6.4h) | **−52%** | 6027 | 11133 | 0 | pencil ~4935s; tip_float 14711→6716; quality trade stated: crevice residuals <0.1mm now left (invisible in oak); finish+pencil SYSTEM now 11.0k→11.0k... total −6444s vs E2 |
| E4 | rough stepovers 1.2→2.4 (both) | 24002 (6.7h) | −49% | — | — | 0 | **WORSE than E3 — REVERTED.** With the chipload modulator already binding (94%+), wider WOC just trades feed down; terrain fragmentation tripled back-rough rapids (19.8→55.2km) and front rough standing-crossings hit 22% at full 4.2mm DPP. The 1.2 stepover was already right. Revert verified: 22911s, OK, 0/0 |
| E5 | optimize_toolpath on Back Rough → applied recommended (DPP 4.2→5.46, stepover 1.2→2.0, RPM 15k→20k, F750→1000, JOINTLY) | **18780 (5.22h)** | **−60.4%** | 6027 | ~7200 | 0 | The optimizer's Ranked winner — gates same-or-better (deflection 57µm unchanged, chipload band-max, power 0.135/0.94kW). What E4 proved impossible one-variable-at-a-time, the joint rebalance delivers: fewer Z levels (25/5.46=5 vs 6) × wider rows × 20k-RPM feed headroom |

## Final state (the `wanaka200_fast` candidate)

**47,412 s (13.2 h) → 18,780 s (5.22 h), −60.4%.** Verdict OK, 0 collisions,
0 rapid collisions, tool-load gates 8/8 Within (real populations), pins keyed.

Deltas from the operator's reviewed `wanaka200.toml` (all four, nothing else):
1. Pencil: hookup_distance 5→15, plunge_rate 135→150 (the tapered-ball cap;
   400 was REFUSED by the flute-tip guard), min_valley_depth 0.05→0.10
2. Finish: drop_cutter stepover 0.3→0.6 (scallop 7.5→30 µm on the R1.5 ball)
3. Back rough: DPP 5.46 / stepover 2.0 / 20,000 RPM / F1000 (optimizer-recommended)
4. Front rough: UNCHANGED (E4 taught us why)

Stated quality trades — the only two:
- open-surface scallop 30 µm instead of 7.5 µm (sub-grain in white oak; sands out)
- crevice residuals under 0.1 mm no longer chased by the pencil (was 0.05)

Files: `wanaka200_fast.toml` (this dir) — the operator's `wanaka200.toml` is
untouched. Machine-ready exports: `~/Downloads/wanaka200/wanaka200_fast_1_Setup_1.nc`
+ `_2_Setup_2___front.nc` (datum headers verified, modulated F-words in the
bytes, M30-complete). The conservative 13.2 h `wanaka200_run_*.nc` files remain
alongside them — the operator picks.

Lessons the scoreboard pins:
- The pencil's ENTRY economics (fed descents at the 150 mm/min flute-tip cap)
  dominated the whole project — 45% of total runtime, invisible in any
  per-op "feeds" view; runtime_by_intent is the instrument that showed it.
- Finish coarsening is TAXED by the pencil's rest detector: every micron the
  finish leaves, the pencil pays for in entries. Tune them as one system.
- Adaptive stepover alone loses to the chipload ceiling (E4); the optimizer's
  joint DPP+stepover+RPM+feed move wins where any single dial fails.

