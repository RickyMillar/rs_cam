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
