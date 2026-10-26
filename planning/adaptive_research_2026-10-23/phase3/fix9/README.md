# fix9: stock maps and readable level images for the G2 columns (C18, 2026-10-26)

Python only, on the existing run artefacts. No cargo, no commit. fix8 and
`fixture3d_v3.html` are not changed. `assert_fix9.py` re-derives every number
below from the raw files with its own code and checks that this file states it
(about 1 minute).

Columns, as on the G2 page: contour_parallel (fix6), zigzag_pick D18 (Corne
fix5; wanaka200 the zigzag run at 31935b89), zigzag_pick D20 (fix7),
AgentSearch (Corne fix1 / fix2 simart; wanaka200 fix2).

## 0. Answers first (regression check and the red rings)

**The "sparse annulus" round (55, 60) is a drawing artefact of the v2 and
v3 pages, not uncut stock.**
- The v2/v3 builder draws each G2/G3 move as its chord: its `parse` reads
  no I/J. On the Z 17 plane of '3D Rough 1' (`arcs.py`, `arcs_z17.json`),
  in the ring 25.45-50 mm round the seed, D20 cuts 42 arcs, 2348.5 mm long,
  which the page draws as 1015.5 mm of chords. D18 cuts 41 arcs, 2360.4 mm
  long, drawn as 1026.2 mm. Inside 25.45 mm the arcs are short (D20 85
  arcs, 790.9 mm drawn as 693.7 mm), so the drawing looks dense there.
  contour_parallel has no arc moves.
- The strict-plane render, which follows the samples, shows continuous
  wraps at the stepover in the ring for both runs
  (`img/compare_corne_D18_D20_Z17.png`). D18 is the run of the v2 page's
  zigzag_pick column (fix5), so the v2 and v3 images show the same artefact
  on the same data.
- **The stock after '3D Rough 1' in that ring** (25.45-50 mm round the
  seed, inside the part outline), height above the target:
  - contour_parallel: mean 0.344 mm, 0.080 of the cells over 0.8 mm;
  - zigzag_pick D20: mean 0.439 mm, 0.093 of the cells over 0.8 mm;
  - zigzag_pick D18: mean 0.997 mm, 0.188 of the cells over 0.8 mm.

  D20 leaves the ring close to contour_parallel. Section 2 shows that Z 17
  cuts it fully.

**The red rings: (b) is true, not (a). The tool really goes down at each
ring.** The G2 page counts only the plunge-entry spans (D20 Corne: 60). The
other descents are inside keep-down-link spans, which the page counts as
keep-down links, not as plunges. Section 4 gives the numbers.

## 1. What the artefacts hold, and how the stock maps are made

- `simulation.json` (`--sim-artifact`) holds only these keys:
  included_toolpath_ids, request_snapshot, resolution_mm, sample_step_mm, schema_version, stock_bbox_max, stock_bbox_min, trace.
  The trace holds the cut samples: the tool tip, the move and the
  engagement, one sample per 0.3 mm of path on Corne and per 1.0 mm on
  wanaka200. **It does not hold the simulated stock**, neither after op 1
  nor at the end. The dexel stock exists only in the CLI process. The
  coverage triage reads that stock and prints its result to `stderr.txt`;
  the full island list of D18 and D20 is in `phase3/fix7/islands_d*.json`
  (D20b, a CLI rebuilt with a larger list cap).
- So `stockmap.py` rebuilds the stock top after the first rough op from the
  samples of toolpath 1. A 6 mm flat end mill lowers each cell whose centre
  is within R = 3 mm of the densified tool path to the tool tip Z. The grid
  is 0.25 mm on Corne and 0.5 mm on wanaka200.
- **The target** is the floor the tool can reach plus the axial leave
  (Corne 0.5 mm, wanaka200 4.0 mm, from the project files). The floor is the
  morphological closing of the part heightmap by a disc of R: the
  drop-cutter height at each centre, then the lowest such height over the
  centres that cover a cell. The part heightmap is rasterised from the
  project's STL (`raster.py`).
- **Frames:** Corne's sim frame is the model frame. wanaka200 setup 0 is
  face bottom up: x = xs - 20, y = 225 - ys, z = 7 - zs. Check: the
  surface minus the tool tip over every D20 cut sample has its 1st
  percentile at 4.00 mm, which is the axial leave.
- **The map** is the stock above the target, inside the part: the filled
  silhouette on Corne, and the terrain footprint inset by R on wanaka200.
  The colour scale is shared per part, from 0 to one depth per pass (6.0
  and 5.46 mm). Rings mark the triage islands. On Corne, the green circle
  marks the D20 seed and the 25.45 mm radius of the C24 spacing law.

Images: `img/stockmap_corne.png`, `img/stockmap_wanaka200.png`.

### Remaining stock after the first rough op, area more than 0.8 mm above the target

| Part | contour_parallel | zigzag_pick D18 | zigzag_pick D20 | AgentSearch |
|---|---|---|---|---|
| Corne, inside the part outline (this rebuild) | 903.7 mm² | 2027.5 mm² | 1042.2 mm² | 3632.6 mm² |
| Corne, coverage triage (CLI, `stderr.txt`) | 1373.5 mm² | 5106.2 mm² | 4095.6 mm² | 6832.7 mm² |
| wanaka200, terrain footprint inset by R (this rebuild) | 2052.2 mm² | 2796.0 mm² | 3073.5 mm² | 6182.8 mm² |

- On Corne, inside the part outline, D20 leaves 1042.2 mm² against
  903.7 mm² for contour_parallel. That is near CP, not 3x.
- The triage's larger numbers come from cells outside the case outline:
  its domain is the floor that each plan can reach, not the part outline.
  The rebuild counts, outside the outline but inside the part's bounding
  box (grey on the map), contour_parallel 1511.3 mm², zigzag_pick D18
  3070.1 mm², zigzag_pick D20 3070.1 mm², AgentSearch 3071.6 mm².
  - The three slice arms leave the same band outside the case wall
    (G-OUTERBAND: the slice regions do not cut it), and contour_parallel
    cuts half of it.
  - So most of the triage's "about 3x" is that outer band. It is the same
    for D18, D20 and AgentSearch, and C31 does not change it.
- So the page's "FLAGGED on all columns" mixes two things: holes inside
  the part, and that outer band.
- wanaka200 prints no island line for any column, so the CLI gives no
  triage number there.
- What the Corne map shows:
  - the crescents in the hex holes are in all four columns;
  - AgentSearch leaves whole hole cores (G-HOLECORE);
  - D18 leaves a dark band inside the frame wall, which D20 does not.

## 2. The circle on Corne D20: cut, not missed

The circle in the level image is centred on the D20 seed, the first plunge
at (55.87, -49.76) in the model frame. That is (55.55, 59.55) in the G-code
frame, where the page's images are drawn.

- **After the first level (Z 17)**, `level1.py` rebuilds the stock from the
  level-17 moves only. It reads the share of the part cells cut down to
  that level's floor, max(target, 17), within 0.8 mm. The cells are in
  rings round the seed, radius 0-25 / 25-50 mm:

  | Column | Cut to the level (0-25 / 25-50 mm) |
  |---|---|
  | contour_parallel | 1.000 / 0.991 |
  | zigzag_pick D18 | 1.000 / 0.995 |
  | zigzag_pick D20 | 1.000 / 0.995 |
  | AgentSearch | 1.000 / 0.986 |

  D20 cuts the whole circle at Z 17. It does not leave it.
- **After the whole op**, the share of the part cells within 50 mm of the
  seed that stand more than 0.8 mm above the target is:
  - contour_parallel 0.080;
  - zigzag_pick D18 0.157;
  - zigzag_pick D20 0.089;
  - AgentSearch 0.304.
- **The part there** is the switch plate. Within 35 mm of the seed:
  - 0.60 of the part-outline cells have no part under them (the through
    switch holes);
  - the rest is the plate, its top at 3.50 mm.

  The circle is not a dome or a bowl. The nearest part surface that stands
  above the Z 17 level is 39.28 mm from the seed.
- **The dense spiral of radius about 25 mm is the C24 spacing law.**
  Inside ρ* = 25.45 mm at stepover 1.2 (`P2_C2_geodesic_fixes.md`), the
  law spaces the wraps closer than the stepover, so they look dense.
  Outside it, the wraps are at the stepover.
- **The "sparse arcs" further out are a drawing artefact.** The v2/v3
  builder draws each G2/G3 arc as its chord (section 0). The images are
  also small: the stepover is about 2 px in a 445 px image. The new
  strict-plane image of D20 at Z 17
  (`img/plane_corne_zigzag_pick_D20_0.png`) shows the wraps as continuous
  lines at the stepover out to the region outline.

## 3. The new images (`img/`)

All images are drawn in the sim frame, from `simulation.json`. Each step
between two fed samples at most 1.5 sample steps apart is drawn: blue for a
cut, orange for a link. Red rings mark the plunge entries, and orange dots
mark the lift-over links that descend into stock (section 4).

- `plane_<part>_<column>_<k>.png`, k = 0, 1, 2: **one strict plane** each
  (Corne Z 17 / 5 / 0.5; wanaka200 Z 19.54 / 8.62 / 4.0). Only steps with
  both ends within 0.01 mm of the plane Z are drawn. The D20 Corne Z 17
  image is 1815 px wide, against 445 px for v3.
- `drape_<part>_<column>.png`: every cut step that is on no plane, coloured
  by Z. This is the drape band.
- `crop_<part>_<column>.png`: a **10 mm square** at the same place for
  every column: Corne (55.1, -44.6) on Z 5.00, wanaka200
  (177.3, 172.3) on Z 8.62. It shows the cut steps from the plane up one depth per pass. The
  square is where the column with the least cut there still has the most.
  The crop image is 821 px wide, so the 1.2 mm stepover can be read. The square is outlined
  in green on each plane image.

## 4. What the red rings mark (the lead's review)

**The old marks.** The first version of `render.py` marked the last sample
of every `entry_plunge` run of the sim trace within 0.35 mm of the plane.
That is 250 marks on the D20 Corne Z 17 image; the G2 page counts 60
plunges for the whole op.

**One meeting-line mark, from program.nc** ('3D Rough 1', D20; G-code Z 0
is model Z 23, so Z -6.000 is the Z 17 level):

```
G1 X74.377 Y59.554 Z-6.000           end of a wrap at the level
G1 X74.377 Y59.554 Z0.500            straight up 6.5 mm (feed 4000)
G1 X75.502 Y59.554 Z0.500 F1519      1.125 mm across, 0.5 mm over the stock top
G1 X75.502 Y59.554 Z-5.500 F750      straight down 6.0 mm at the plunge feed
G0 X75.502 Y59.554 Z-5.000           peck: up 0.5 mm
G1 X75.502 Y59.554 Z-6.000 F750      down to the level
G1 X75.097 Y63.554 Z-6.000 F4000     the next wrap
```

The sim trace gives the moves the same intents: 518 `linking` (up),
519 `linking` (across), 520 `entry_plunge` (down 23.35 → 17.65,
24.92 mm³ removed), 521 `retract`, 522 `entry_plunge` (2.54 mm³). They
are all in one semantic span with the style "keep-down link".

So the tool goes down into stock there. The span is a **lift-over link**:
the link rises over the uncut stock to 0.5 mm above the stock top, and
then plunges back down 6 mm at the plunge feed.

**Counts per column** (`entries.py`, `entries.json`). A span counts as a
descent into stock when its `entry_plunge` samples drop 1 mm or more and
remove stock (removed volume > 0.1 mm³).

| Column | Plunge-entry spans (G2 page "plunges") | of them a descent into stock | Keep-down-link spans that descend into stock | On the Z 17 plane: plunge entries / lift-over descents |
|---|---|---|---|---|
| Corne contour_parallel | 73 | 27 | 13 | 5 / 1 |
| Corne zigzag_pick D18 | 56 | 34 | 719 | 6 / 156 |
| Corne zigzag_pick D20 | 60 | 36 | 1195 | 9 / 238 |
| Corne AgentSearch | 48 | 31 | 127 | 7 / 23 |
| wanaka200 contour_parallel | 964 | 257 | 123 | — |
| wanaka200 zigzag_pick D18 | 22 | 11 | 2622 | — |
| wanaka200 zigzag_pick D20 | 20 | 14 | 3578 | — |
| wanaka200 AgentSearch | 32 | 26 | 481 | — |

- The D20 Corne lift-over descents have a median drop of 5.26 mm. Each
  removes a median of 6.974 mm³.
- On Corne D20, 40 of them are on the meeting line (y = -49.76).
- **The G2 page's plunge count is right for what it counts** (the
  plunge-entry spans), but it does not show these descents. On the
  geodesic arms they are about 20 times the plunges on Corne (1195
  against 60 for D20) and more than 100 times on wanaka200 (3578 against
  20).
- This is a finding for the G2 decision. Each such descent is a vertical
  feed into stock with a flat end mill, up to one depth per pass.

**The render now** (`render.py`):
- a red ring marks only the end of a plunge-entry span on the plane;
- a small orange dot marks a lift-over link that descends into stock and
  ends on the plane;
- each plane image's title gives both counts.

I re-ran the images and `assert_fix9.py`.

## Files

| File | What |
|---|---|
| `raster.py` | STL to top / bottom heightmap (`corne_part.npz`, `wanaka_part.npz`) |
| `stockmap.py` | the stock after op 1 and the target (`stock_<part>_<column>.npz`) |
| `level1.py` | the Corne stock after level Z 17, in rings round the seed (`level1_rings.json`) |
| `render.py` | all images in `img/`, `crop_centres.json` |
| `arcs.py` | arcs and chords on the Z 17 plane (`arcs_z17.json`) |
| `entries.py` | every entry span, its descent and the stock it removes (`entries.json`) |
| `assert_fix9.py` | own rebuild of every number above; checks this file (`assert_values.json`) |
