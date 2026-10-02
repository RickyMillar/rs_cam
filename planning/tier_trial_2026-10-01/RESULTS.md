# Tiered finishing trial on the 350 board — results (2026-10-02)

Plan, pass rule and amendments: `PLAN.md` (written before the runs).
Instrument: `crates/rs_cam_core/tests/tier_trial_350.rs` (one arm per run)
and `crates/rs_cam_core/tests/tier_trial_blob_preview.rs`. Table:
`results.csv`. Every claim below is asserted by `analyse.py` (run it from
the repo root). Run JSONs and maps: `runs/`.

Board: the rivmap100 terrain x3.5 (350 x 350 x 42 mm, white oak),
`planning/fixtures/rivmap100/rivmap100_memory_repro.toml`. The operator's
own `rivmap350.toml` is not in the repo yet; re-run the winners on it.
Times are the kinematic cycle time of the finish ops (machine profile
acceleration 500 mm/s², rapids and links included). Feeds are the Suggest
feeds (vendor LUT) for every op. Quality is pointwise: 16 windows of
25 x 25 mm at 0.05 mm cells, plus the whole board at 0.25 mm.

## Answer

**The cutting style matters far more than the tool ladder.** A fine
parallel raster with the R1 finishes the board faster than the R1 iso
scallop at the same or better quality:

| Finish (R1 tapered ball) | Time | p99 | area > 0.06 | gouge | board > 0.3 mm | entries | Pass |
|---|---:|---:|---:|---:|---:|---:|---|
| Iso scallop h 0.03 (reference) | 11.93 h | 0.249 | 12.1 % | 0.10 % | 1 045 mm² | 613 | yes |
| Raster, stepover 0.21 (cusp held to 65°) | **6.75 h** | 0.259 | 10.2 % | 0.00 % | 1 100 mm² | 1 | yes |
| Raster, stepover 0.17 (cusp held to 70°) | **8.36 h** | 0.246 | 8.5 % | 0.00 % | 996 mm² | 1 | yes |
| Raster, stepover 0.24 (cusp held to 60°) | 5.72 h | 0.275 | 12.4 % | 0.00 % | 1 220 mm² | 1 | p99 only |

The raster stepover comes from geometry, not tuning: on a slope θ the
surface spacing is the XY stepover / cos θ, so s = 2·√(2Rh − h²)·cos θ
holds the 0.03 cusp up to θ. Raster time follows 1 / stepover within 15 %.

**Tiering does not pay on this terrain**, with one exception:

- Every R2 → R1 tier arm (tolerance 0.05 to 0.25) is slower than the
  reference or fails the finish. The fine tier splits into 3 900 to 6 600
  islands; their entries and travel cost 4-7 h.
- The operator's "blob" idea (a Ø6 ball, R3, takes the flats and the sea;
  the R1 takes the whole range as one region, island close radius 5 mm)
  is the only tier arm faster than the reference at near-equal finish:
  9.79 h, p99 0.251, gouge 0.05 %. It fails one margin (area > 0.06:
  14.0 % vs 12.1 %). On this board the range is about 80 % of the area,
  so the big ball can only take about 20 %. The R3 ball is hypothetical.
- R2 alone (7.1 h) cannot reach the valleys: p99 0.67.
- Pencil cleanup after R2, R2 skipping the R1 islands, and an R2
  semi-finish all lose.

## Why the iso scallop is slow

It asks for 4 105 mm/min and averages about 800 mm/min. The machine's
acceleration limits 65 % of its moves (tight curves). Its path is also
1.4x longer than a cascade's (it packs passes on slopes to hold the
cusp), and its 613 entries cost 1.27 h. The raster runs straight lines
at a LOWER feed (2 743 mm/min, a different LUT row: SpeTool parallel vs
Amana v8) and still wins.

## Defects found and fixed during the trial

- **G-FLUTETOP (`cd7c75f4`, safety).** The push cutter ignored mesh more
  than the flute length above a waterline level, so a waterline (and the
  steep band of Unified Finish, the GUI tier default) cut FED loops
  inside hills taller than the flutes. T5 (GUI-default tiers) cut to the
  stock floor: min dev −21 mm → −0.45 mm after the fix.
- **G-ARCFITSCAN (`bd90ffdd`, performance).** Arc fitting rescanned the
  run from every start move: N² on a raster. R1 raster generation at
  stepover 0.243: > 3 h → 49 s. Output unchanged.
- **Arc tolerance gouges.** The fixture's arc tolerance 0.05 gouged 8 %
  of the surface 0.02-0.08 mm deep. The tier rule (cusp / 2 = 0.015) does
  not: 0.10 %. 0.02 gives 0.16 %, 0.03 gives 1.76 %. Every arm here ran
  with cusp / 2 (Amendment 1).

## Open

1. **Unified Finish tiers leave the flanks unfinished** (T5 after the
   fix: 22 745 mm² with more than 0.3 mm left, 19 % of the model).
   Investigation in progress. Until it is fixed, prefer IsoScallop tiers
   or a single-tool finish on tall terrain.
2. **Column deviations hide deep gouges**: `compute/simulate.rs` drops a
   column more than max(0.5 x model thickness, 2 mm) from the model.
3. **G-code size.** The R70 raster is 4.4 M moves (iso: 1.4 M). Arc
   fitting cannot shrink straight lines; check the sender copes before a
   run.
4. **Raster direction.** The raster runs along X only; a slope facing X
   gets the largest cusp. A raster angle (or a cross raster) is untested.
5. **Next arm:** R60 raster + an R1 iso scallop on the steepest ground,
   welded into one region (the blob idea applied to slope), to beat R65.
6. Re-run the winners on the operator's `rivmap350.toml` and real tools.

## Operator decisions

- Default arc tolerance for finish ops: 0.05 → cusp / 2 (0.015 at h 0.03)?
  It costs 2.3 h on this board, and removes 8 % gouge cover.
- Is a fine raster finish acceptable to the eye? The maps
  (`runs/R65_p_w*.png`, `runs/R70_p_w*.png` vs `runs/A1_planner_w*.png`)
  are the evidence; the pass rule is a judgement call (PLAN.md).
