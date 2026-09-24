# Entry probe on rivmap100 (2026-09-24)

Research measurement only. The probe code is not committed; the worktree
was reverted after the runs. Base: master `a9a6a2b6`.

Scope change (operator ruling, 2026-09-24): the step ladder goes. This note
therefore measures the ENTRIES of plain single-step 3D Roughs. The ladder
and fragmentation numbers from before the ruling are in the last section.

## Method

- Instrument: `rough-score <demo> --toolpath 1 --resolution 0.5`, on
  `scratchpad/demo/rivmap100_ladder_demo.toml` (not edited), with
  `--set '1.coarse_steps=[]'` and the Depth/Pass of the arm. The demo
  op uses Contour Parallel, `entry_style = "helix"`, plunge rate 500 mm/min,
  helix radius 0.3 × 6 = 1.8 mm, pitch 1 mm.
- A throwaway hook in `adaptive3d/clearing.rs` recorded each planner
  `Rapid` / `Link` segment before its stamp, with the PLANNER dexel stock
  at that moment. An entry "touches open space" when a cell within
  R + stepover (4.2 mm) of the entry point has a stock top at or below the
  entry Z + 0.1 mm. "Footprint material" is the highest stock top within R.
- A throwaway dump in `compute_cycle_time_breakdown` wrote the time of each
  emitted move (the accel integrator that `rough-score` uses).
- Classification by geometry: an arc descent is a helix, a descent with XY
  travel is a ramp, a vertical fed descent is a plunge. Peck steps at one
  XY are one entry. A fed vertical descent with a non-entry intent is an
  untagged descent. Emitted entries join planner records by XY (0.2 mm);
  3 shallow entries (0.5 mm) per arm do not join.

## Result: entry count and time, helix style (the demo setting)

| Arm | Total s | Entries | Entry s | Plunge part s | Helix part s | Entry s above material |
|---|---|---|---|---|---|---|
| dpp 2, By Area | 1 379 | 218 | 538 | 135 | 399 | 435 (81 %) |
| dpp 4, By Area | 864 | 141 | 351 | 82 | 265 | 248 (71 %) |
| dpp 8, By Area | 586 | 83 | 242 | 54 | 184 | 146 (60 %) |
| dpp 8, Global | 637 | 92 | 272 | 62 | 206 | 173 (64 %) |

- Every entry has the same shape: a fed straight plunge from Z 14.0 to
  2 mm above the entry Z, then a 2 mm helix (2 turns of 11.3 mm at
  500 mm/min, about 1.8 s). All plunges start at Z 14.0 (206, 132, 77
  and 86 of them).
- Count by type: helix entries 218 / 141 / 83 / 92. Ramp 0. Pure plunge 0
  (the plunge part is inside each helix entry). Stay-down link 0.
  Untagged vertical descent 0. The `rough-score` counter
  `entry_runs_by_kind` reads the intent of the FIRST move of a run, so it
  reports most of these as `entry_plunge` (77 of 84 on dpp 8).
- "Above material" is time-accurate: for each move, the share of its Z
  range above the footprint material, times its integrated time.

## Why the helix entry falls back to a plunge

Two code paths, both on the Contour Parallel door:

1. `adaptive3d/path.rs:1693` (Helix arm of `segments_to_toolpath`) rapids
   to `safe_z` and calls `dressup::emit_helix` with
   `entry_safety(params.safe_z)` (line 1712). The stock-top guard is
   `safe_z`, because a plain `Rapid` segment carries no floor.
   In `dressup/entry_descent.rs:627`,
   `safe_rapid_floor = stock_top + ENTRY_CLEARANCE` = safe Z + 2, so
   `rapid_target_z` = safe Z: no rapid. Line 639 then FEEDS an
   `EntryPlunge` from safe Z down to `end.z + ENTRY_CLEARANCE` (2 mm). Only
   the last 2 mm is a helix.
2. `session/compute.rs:828` calls `dressup::optimize_entry_descents_annotated`
   with `gen_initial_stock`, the op's SEED stock (after the Face, top
   Z 12). `dressup/mod.rs:599-600` splits the plunge at the seed stock top
   + `PLUNGE_CLEARANCE_MM` (2) = Z 14.0 and rapids down to it. It does not
   see the stock that the op itself has cut, so on every deeper level the
   fed plunge runs through air that the op cleared.

`RapidWithFloor` (a rapid down to the prior floor) exists, but only the
AgentSearch arm emits it (`clearing.rs:2088` and later). Contour Parallel
emits plain `Rapid`. The keep-down link runs only for `Plunge` style
(`path.rs:1650`, `1762`), so helix style never gets one.

## Result: open space at the entry Z

| Arm | Open entries | Open entry s | Est. save s | Rapid before open entries s |
|---|---|---|---|---|
| dpp 2, By Area | 174 of 218 (80 %) | 400 of 538 (74 %) | 344 | 153 |
| dpp 4, By Area | 118 of 141 (84 %) | 291 of 351 (83 %) | 253 | 103 |
| dpp 8, By Area | 64 of 83 (77 %) | 197 of 242 (82 %) | 175 | 57 |
| dpp 8, Global | 73 of 92 (79 %) | 228 of 272 (84 %) | 203 | 67 |

- The median distance from an open entry to the nearest open cell is
  0.0 mm. The entry cell itself is already cut to the entry Z. The contour
  loop starts at its lowest stock point, which the ring or level before
  has usually cleared. Thus a side entry needs no horizontal lead: the
  saving is the descent itself.
- Estimate assumptions: each open entry becomes a feed move from the
  nearest open cell (2 400 mm/min) plus a rapid Z descent (Z accel
  270 mm/s², cap 166 mm/s). The retract and the rapid before the entry
  stay; they are the last column, a further saving only if the tool
  stays down.
- Estimated totals with the saving: dpp 8 586 → about 411 s (−30 %),
  dpp 4 864 → 611 s, dpp 2 1 379 → 1 035 s.
- The closed entries (20 to 23 %) are the first entries into uncut stock
  at each level. dpp 8: 20 entries descend more than 5 mm into material.
  These need the helix.

## Measured check: `--set 1.entry_style=plunge`

This style turns on the existing keep-down link (8 × D) and the peck
ladder. It is a measurement, not the estimate above.

| Arm | Helix total s | Plunge total s | Entry s | Link s | Plunges | Keep-down descents |
|---|---|---|---|---|---|---|
| dpp 2, By Area | 1 379 | 1 072 (−22 %) | 137 | 105 | 56 | 88 |
| dpp 4, By Area | 864 | 608 (−30 %) | 55 | 75 | 35 | 71 |
| dpp 8, By Area | 586 | 394 (−33 %) | 29 | 64 | 22 | 45 |
| dpp 8, Global | 637 | 418 (−34 %) | 31 | 64 | 23 | 53 |

- Each keep-down link ends in a fed vertical descent tagged `Linking`,
  not as an entry. That is the descent class that `dressup/CLAUDE.md`
  names. It exists only on this arm.
- CAUTION: plunge style drives a flat end mill straight down into
  material. On dpp 8, 4 plunges go 8.0 mm into stock. The keep-down link
  tests its corridor against the surface; this probe did not test it
  against the stock. Do not ship this arm as a default before both are
  checked.

## Conclusions

1. On single-step roughs, 60 to 81 % of the entry time is fed motion
   above the material, and 74 to 84 % of it is at entries whose own cell
   is already open at the entry Z.
2. The cause is two stock-top guards that read the wrong stock: the
   helix emitter reads `safe_z`, and the descent optimiser reads the op's
   seed stock. Neither reads the planner stock that the op has cut.
3. The fix candidates, for an operator ruling: (a) give the Contour
   Parallel entry the planner's local stock top (a `RapidWithFloor`, as
   AgentSearch does), so the rapid stops 2 mm above the real material;
   (b) skip the helix where the entry cell is already open, or take the
   keep-down link for helix style too. Estimate −30 % on dpp 8; the
   plunge-style arm measures −33 %.

## Before the ruling: the ladder and the 39 parts (kept for the record)

- Margin 0, `[8]` + 2 reproduced the RESULTS.md probe: 11 093 moves,
  1 728 s, entry 951 s, 39 parts at Z 4.0. 92 % of its entries were open
  (853 of 951 s); 651 s of entry time was above the material.
- The 39 parts were 6 valley-floor parts and 33 gentle shelves (floor
  inside the slab at Z 4.3 to 11.9, slope < 30°) that fit rule v2 admits.
  21 parts were smaller than one tool disk (28.3 mm²). The real valleys
  below Z 4.0 were 11 closed basins, 370 mm² in total (258 and 66 mm² are
  the only useful two). The model rim at Z 12 closes every valley.
- The splitter was isolated steep cells at 0.5 mm. An opening of the
  steep mask by 1 cell took 39 parts to 5; slope 40° to 3; slope 0°
  (fit rule v1) to 4, all on valley floors. A minimum area and a margin
  of R also cut the count.
- No dial made the ladder beat dpp 8. Best rows: opening 1, `[8]` + 4,
  907 s (helix) and 638 s (plunge style), against dpp 8 at 586 / 394 s.
  The base tier carries the time; the part count does not.
- Pictures: `scratchpad/vp/png/margin0_z4.png`, `slope0_z4.png`,
  `open1_z4.png`, `smooth2_z4.png`.

Scratch data: `scratchpad/vp/runs/<arm>/` (score, moves, entries).
