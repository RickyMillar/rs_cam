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


## Morning A/B (operator asked): unified finish vs the tuned drop_cutter — 2026-08-23

Question: "unified finish instead of a pencil, and a rougher first finish?"

**Answer, measured**: unified replacing the DROP_CUTTER (pencil kept) wins
modestly; unified replacing the PENCIL was not tested because the pencil is
rest-targeted and unified is slope-targeted — on this terrain the steep band
is far larger than the actual leftover, and the pencil's cost is already
down 4.7×. "Rougher first finish" with the same tool is a wash (work moves
between passes; total path for the final quality is conserved).

| finish strategy | total | finish op | pencil after | gates |
|---|---|---|---|---|
| drop_cutter 0.6 (E5 candidate) | 18780s (5.22h) | 6027s | 14.2km cut | 8/8 Within |
| unified 0.6/0.3/sc0.1 (bottom_z FIXED) | **17697s (4.92h)** | ~7000s incl. waterline | 12.0km cut | 8/8 Within |

Why unified wins now (different reason than the airrun's comparison): at
0.6-stepover the drop_cutter is no longer accel-bound, but unified matches a
DIFFERENT vendor row (`amana-tapered-hardwood-scallop-3175-2f`, semi_finish,
band max 0.0247 vs parallel row's 0.0201) → ~23% higher clamped feed, plus
banding path savings, minus the waterline band it must now actually cut.

**Two findings out of this A/B:**

- **G-UNIFIEDCRASH**: unified_finish generation panics ("index out of
  bounds: the len is 0 but the index is 1", caught by the worker) at
  scallop_height 0.03 + z_step 0.6 (raster 0.6); identical input generates
  fine at scallop 0.1 + z_step 0.3. Param isolation not yet done. Repro
  TOMLs in the session scratchpad.
- **G-UNIFIEDBOTTOMZ**: with heights AUTO, the resolved bottom_z = 7.0 (the
  stock TOP in the emission frame) clips the VerySteep waterline band's Z
  range to nothing — the band emits ZERO cutting, silently but for the
  `unmachined_band` report (2,681.5 mm², the whole band). The narration
  names the fix ("pin bottom_z"), which worked (bottom_z −3 → band cuts,
  unmachined none). **This also invalidates the airrun's unified arm**: its
  byte-identical 2,681.5 mm² means THAT unified never cut its steep band
  either — its runtime was underpriced and its comparison to drop_cutter
  was apples-to-broken. Auto-bottom_z resolving to the stock top for a
  finish op that must ladder below it looks like a real heights-resolution
  defect, not operator error.

Files: `wanaka200_fast_unified.toml` + `~/Downloads/wanaka200/
wanaka200_fast_unified_*.nc` (verified M30-complete, modulated F-words).
Three programs now on disk: conservative 13.2h, drop_cutter 5.22h,
unified 4.92h.

## Efficiency campaign log — 2026-08-23 (C-series, plan in EFFICIENCY_CAMPAIGN_2026-08-23.md)

**C0 baseline** (fresh GUI, unified candidate): 17,588 s, OK, 0/0. Per-op
runtime_by_intent: finish 5,636 (cutting 4,616), back rough 4,422 (entries
900), **pencil 4,126 (entries 3,244 — back on top as the #1 lever)**, front
rough 2,570 (54% air), rivers 707 (cutting 158). Campaign re-ranked
accordingly: C6-early → C2 → C5 → C3 → C4.

**C1 height-ladder audit — the operator's "levels above the stock" SOLVED:**
the Heights panel shows only the five reference planes (clearance/retract/
feed/top/bottom), all expressed "above Stock Top" and drawn floating above
the stock sketch; the actual cut ladder never appears there. Emitted motion
has NO air levels (back rough first pass 19.54 = 25 − DPP, front 2.8 =
7 − DPP). Presentation quirk, not waste. UI suggestion for later: show the
resolved cut ladder in the panel, and label the planes as travel planes.

**G-HEIGHTSTAB — filed (severity: part-state-destroying, silent).**
Opening the Heights properties tab via `set_ui_view(toolpath_index=1,
properties_tab="heights")` marked Back Rough stale and auto-regenerated it
to **ZERO moves** — op params verified untouched (optimizer values intact),
so the mutation went through the heights channel; the panel displayed
`Bottom: −0.0 mm above Stock Top = 25.0` (the G-UNIFIEDBOTTOMZ family), and
committing that pinned bottom at the stock top clips the whole op. Blast
radius observed before recovery: 262 rapid collisions as every setup-1 op
rapids through the never-roughed back; sim total misleadingly "improved" to
12,362 s because 4.4k s of roughing vanished. **A gate that reads "generated
0 moves" as Done is what let this sail** — `zero_removal`/generated-empty is
report-only. Mechanism (tab-open commits a heights draft?) needs a code
session to confirm; hypothesis strong. Workaround: never open the Heights
tab on a healthy op; recover by project reload.

**C6 (pencil hookup 15→30) — ACCEPTED: 17,588 → 16,793 s (−794 s).**
0/0 collisions, back rough verified intact after the clean reload (7,672
moves), pencil tip_float byte-identical (5,418) and cutting distance −3%
(removed re-approach ramps only) — valley coverage untouched.
