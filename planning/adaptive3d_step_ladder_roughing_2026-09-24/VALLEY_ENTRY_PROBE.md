# Valley-entry and fragmentation probe on rivmap100 (2026-09-24)

Research measurement only. The probe code is not committed; the worktree
was reverted after the runs. Base: master `a9a6a2b6`.

## Questions

1. The operator: every valley that a 6 mm bit fits into is a pocket. The
   margin-0 probe found 39 coarse parts and was slower than one step.
   Hypothesis: the entries, not the detection, cost the time. Most parts
   touch open space and can take a side entry.
2. The operator (addition): 39 is over-fragmentation; expect fewer than 10
   pockets. Find the cause, test dials, and compare with dpp 8 (586 s).

## Method

- Instrument: `rough-score <demo> --toolpath 1 --resolution 0.5`, on
  `scratchpad/demo/rivmap100_ladder_demo.toml` (not edited). The release CLI
  was built in the worktree.
- A throwaway hook in `adaptive3d/clearing.rs` read env dials (link margin,
  wall slope, slope smoothing, keep-out opening, minimum part area). It
  recorded each planner `Rapid` and `Link` segment before its stamp.
- The open test uses the PLANNER dexel stock at the time of the entry. An
  entry "touches open space" when a cell within R + stepover (4.2 mm) of
  the entry point has a stock top at or below the entry Z + 0.1 mm.
- A throwaway dump in `compute_cycle_time_breakdown` wrote the time of each
  emitted move (the accel integrator of `rough-score`). Entries are
  classified by geometry: an arc descent is a helix, a descent with XY
  travel is a ramp, a vertical descent is a plunge. Peck steps at one XY
  are one entry. A fed vertical descent outside an entry run is an
  untagged descent. Emitted entries join planner records by XY (0.2 mm).
- Reproduction: margin 0, `[8]` + 2 gives 11 093 moves, 1 728 s, entry
  951 s and 39 parts at Z 4.0. These equal the RESULTS.md probe row.

## Result 1: the entries are fed air, not pocket entries

Helix style (the demo setting). Every entry is a helix (and 6 to 7 short
ramps). There is no plunge, no keep-down link and no untagged descent.
`segments_to_toolpath` tries the keep-down link for `Plunge` only, so
with helix entries it never runs.

| Arm | Total s | Entries | Entry s | Open entries | Open entry s | Est. save s |
|---|---|---|---|---|---|---|
| `[8]` + 2, margin D (master) | 1 379 | 218 | 538 | 174 (80 %) | 400 (74 %) | 344 |
| `[8]` + 4, margin D (master) | 864 | 141 | 351 | 118 (84 %) | 291 (83 %) | 253 |
| `[8]` + 2, margin 0 | 1 728 | 454 | 951 | 418 (92 %) | 853 (90 %) | 725 |
| `[8]` + 4, margin 0 | 1 314 | 358 | 654 | 330 (92 %) | 586 (90 %) | 488 |
| dpp 8, one step | 586 | 83 | 242 | 64 (77 %) | 197 (82 %) | 175 |

Per tier, margin 0, `[8]` + 2: coarse 301 helixes, 561 s, 284 open;
base 135 helixes, 374 s, 128 open.

- The median distance from an open entry to the nearest open cell is
  0.0 mm. The entry point itself is already cut to the entry Z. The
  contour loop starts at its lowest stock point, which the ring before has
  usually cleared.
- 414 of 454 helixes (margin 0, `[8]` + 2) start at Z 14.0, the stock BOX
  top. The Face took the stock to Z 12, and the local stock is lower. The
  mean drop is 6.4 mm; the mean drop above the material is 5.4 mm. By a
  Z-proportional split, 81 % of the entry time (771 of 951 s) is a fed
  helix in air at 500 mm/min. For dpp 8 it is 70 % (170 of 242 s).
- Only 26 coarse entries descend more than 5 mm into material (64 s).
  These are the first entries into a closed part. They need a helix.
- Estimate assumptions: an open entry becomes a feed move from the nearest
  open cell (2 400 mm/min) plus a rapid Z descent (Z accel 270 mm/s², cap
  166 mm/s). The retract and the rapid before the entry stay. That rapid
  transit is a further 364 s (`[8]` + 2), 282 s (`[8]` + 4) and 57 s
  (dpp 8) before the open entries.

The hypothesis is confirmed in its time share: about 90 % of the entry
time is at open-touching entries. The mechanism is not a side entry. The
entries descend through air that the planner has already cleared.

### Measured check: `--set 1.entry_style=plunge`

This style turns on the existing keep-down link and the peck ladder.

| Arm | Helix total s | Plunge total s | Plunge: entry s, link s |
|---|---|---|---|
| dpp 8, one step | 586 | 394 (−33 %) | 29, 64 |
| `[8]` + 4, margin D | 864 | 608 (−30 %) | 55, 75 |
| `[8]` + 4, margin 0 | 1 314 | 984 | 175, 252 |
| `[8]` + 2, margin 0 | 1 728 | 1 337 | 305, 313 |

With plunge style, 105 to 643 fed links hold 46 to 257 s. Each link ends
in a fed vertical descent with the `Linking` intent (45 to 222 of them),
which is the untagged descent that `dressup/CLAUDE.md` names.

CAUTION: plunge style drives a flat end mill straight down into material.
On dpp 8, 4 plunges go 8.0 mm into stock. The keep-down link tests
its corridor against the surface; this probe did not test it against the
stock. Do not use this arm as a product setting before both are checked.

## Result 2: what the 39 parts are

Margin 0, level Z 4.0 (slab 12 → 4), planner cell 0.5 mm. Picture:
`scratchpad/vp/png/margin0_z4.png` (parts in colour, keep-out in red, blue
line = floor at or below Z 4.0).

- 6 parts hold a valley floor below Z 4.0. The other 33 parts are gentle
  shelves (floor inside the slab, slope < 30°) at Z 4.3 to 11.9. Fit rule
  v2 admits them. They are not valleys.
- Sizes: 21 parts are smaller than one tool disk (28.3 mm²), 11 are 1 to
  4 disks, 4 are 4 to 10 disks, 2 are 10 to 40 disks, and 1 is larger
  (1 749 mm², a shelf 50 × 83 mm at Z 5.1 to 10.5).
- The real valleys: the cells with floor at or below Z 4.0 inside the
  model form 11 components of 258, 66, 21, 10, 6, 5, 2, 1, 1, 0 and 0 mm².
  Only two are larger than 4 tool disks. (A 12th component, 882 mm², is
  the off-mesh border ring, which the border clear cut first.)
- The keep-out web: the steep cells (≥ 30°) along every bank make a fine
  web. Isolated steep cells split one shelf into many parts (129
  floor-mask parts before the margin and the drop).
- Open or closed: 0 of 39 parts touch cut ground at or below Z 4.0 when
  the level starts, and 0 of the 11 valley components connect to the cut
  border ring. The model rim stands at Z 12 around the terrain. Every
  valley is a closed basin at the coarse level.
- The second clip level (Z 0.5, slab 4 → 0.5) has 4 parts (87 mm²).

## Result 3: dials, one at a time on margin 0

Parts = coarse parts at Z 4.0 (of which on a valley floor). Times in s.

| Dial | Parts (valley) | Area mm² | `[8]`+2 total / entry | `[8]`+4 total / entry |
|---|---|---|---|---|
| none (slope 30°) | 39 (6) | 4 557 | 1 728 / 951 | 1 314 / 654 |
| slope 40° | 3 (1) | 5 923 | 1 704 / 928 | 1 670 / 909 |
| slope 50° | 1 (1) | 7 184 | 1 734 / 995 | 1 734 / 995 |
| slope 0° (fit rule v1) | 4 (4) | 354 | 1 541 / 700 | 964 / 436 |
| slope mean 3 × 3 | 25 (4) | 4 690 | 1 571 / 854 | 1 259 / 649 |
| slope mean 5 × 5 | 12 (4) | 4 915 | 1 515 / 825 | 1 201 / 626 |
| steep-mask opening 1 cell | 5 (3) | 5 434 | 987 / 461 | 907 / 410 |
| steep-mask opening 2 cells | 1 (1) | 6 287 | 988 / 467 | 960 / 454 |
| min part area 1 disk | 18 (3) | 4 268 | 1 737 / 906 | 1 520 / 781 |
| min part area 4 disks | 7 (1) | 3 668 | 1 733 / 864 | 1 376 / 677 |
| margin R (0.5 D) | 3 (1) | 133 | 1 503 / 623 | 951 / 404 |
| margin D (master) | 0 | 0 | 1 379 / 538 | 864 / 351 |

- The operator asked for a "closing of the keep-out". A closing fills the
  gaps in the keep-out and adds fragments. The probe used an OPENING of
  the steep mask (the same as a closing of the floor mask). It removes
  isolated steep cells. The "above the slab" cells did not change.
- The cause of the count: (a) fit rule v2 admits every gentle shelf in the
  slab (33 of 39 parts); (b) isolated steep cells at 0.5 mm split the
  shelves (one opening cell takes 39 to 5). A minimum area removes small
  parts but does not join the large ones.
- The part count does not set the time. With 1 part (opening 2) the run
  takes 960 to 988 s. The entry time stays 45 to 50 % of the total.

## Answer to question 3

- Setting that matches the real valleys: slope 0° (fit rule v1: only cells
  whose floor is at or below the level), margin 0. It gives 4 parts, all
  on valley floors (258, 66, 21 and 10 mm²). Picture:
  `scratchpad/vp/png/slope0_z4.png`. Opening 1 cell gives 5 parts, but
  one is a 5 271 mm² shelf web (`png/open1_z4.png`).
- It does NOT beat dpp 8. `[8]` + 4: 964 s (helix), 691 s (plunge style),
  against dpp 8 at 586 s (helix) and 394 s (plunge style). No dial row
  beats dpp 8. The best ladder row is opening 1, `[8]` + 4, plunge style:
  638 s.
- Entry classification, slope 0°, `[8]` + 4, helix: 175 helixes, 436 s;
  154 (88 %) touch open space, 378 s (87 %); est. save 329 s. Coarse
  tier: 8 helixes, 25 s, mean drop 9.9 mm. Base tier: 164 helixes, 407 s.
- The reason: the base tier carries the time. A ladder with base 4 costs
  at least the dpp 4 base work outside the coarse parts (dpp 4 alone:
  864 s). The real valleys below Z 4 cover about 370 mm², 3.7 % of the 100 × 100 mm plan area.

## Conclusions

1. Entries cost the time, as the operator's hypothesis says. The fix is
   not a side entry. The helix starts at the stock box top (Z 14.0) and
   feeds down through cleared air. A rapid to the local stock top (or the
   existing keep-down link) removes most of it: measured −33 % on dpp 8.
2. The 39 parts are 6 valley floors and 33 gentle shelves. The real
   valleys at Z 4 are 2 useful pockets and 9 crumbs, all closed basins.
3. On rivmap100 no fragmentation dial makes the ladder beat dpp 8. A
   ruling on the entry style comes first; the fit rule after it.

Scratch data: `scratchpad/vp/runs/<arm>/` (score, moves, entries, parts).
