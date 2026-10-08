# RivMap oak block: the two-sided rs_cam job (2026-10-08)

Scope: the rs_cam job for the RivMap oak block `nz-south` (back, epoxy pour,
skim, flip, front) and the same method for the epoxy test piece. The brief
is `project_rivmap_mono/manufacture/docs/rs_cam_flip_setup_prompt.md`; the
lead and the operator set the method below, and the method wins where the two
differ.

Nothing in this package is for the machine. The G-code that the checks read
was emitted to a scratchpad for measurement only.

## Files

| File | What it holds |
|---|---|
| `planning/rivmap_block_job/derive.py` | Reads the block-stage output (read only) and writes the models and both project files. A second run writes the same bytes (checked with `sha256sum`). |
| `planning/rivmap_block_job/nz_south.toml` | The nz-south project: 3 setups, 33 toolpaths, 6 tools. Generated; do not edit. |
| `planning/rivmap_block_job/testpiece.toml` | The test-piece project: 3 setups, 11 toolpaths, 3 tools. Generated. |
| `planning/rivmap_block_job/check_gcode.py` | Checks 1 to 3 on emitted nz-south G-code. |
| `planning/rivmap_block_job/check_testpiece.py` | The web and frame checks on emitted test-piece G-code. |
| `planning/rivmap_block_job/split_gcode.py` | Splits a program by setup and by toolpath for `rs_cam_cli nc-time`. |
| `/home/ricky/cnc_jobs/rivmap_block/nz_south/` | One DXF per operation (back-view frame), `terrain_s1.stl` (setup-1 frame), outside both repositories. |
| `/home/ricky/cnc_jobs/rivmap_block/testpiece/` | The same for the test piece. |

Commands:

```
uv run --with ezdxf --with numpy --with shapely python planning/rivmap_block_job/derive.py
rs_cam_cli project planning/rivmap_block_job/nz_south.toml --setup "1 Back" --output-dir D --emit-gcode X.nc
uv run --with ezdxf --with numpy --with scipy --with shapely python planning/rivmap_block_job/check_gcode.py planning/rivmap_block_job/nz_south.toml X.nc out.json
python3 planning/rivmap_block_job/split_gcode.py planning/rivmap_block_job/nz_south.toml X.nc split/ && rs_cam_cli nc-time split/*.nc
```

## What rs_cam does and does not do for a two-sided job

It does:

- One modelled flip: `FaceUp::Bottom` maps a world point to
  `(x, D - y, H - z)` in stock-relative coordinates
  (`crates/rs_cam_core/src/compute/transform.rs:116`). This is a rotation of
  180° about a line parallel to machine X at `y = D/2`. The XY of the flipped
  frame is the same machine point, so one XY zero serves both sides.
- A 2D drawing on a Bottom setup is mirrored in Y and its rings are re-wound
  (`compute/transform.rs:363`, `:472`).
- Alignment pins on the stock, a keyed-pair rule and a load warning. A pair
  is keyed when both pins are on `y = D/2` and the X positions miss mirror
  symmetry by `PIN_KEYING_ASYMMETRY_MM` = 3.0 (`compute/alignment_pins.rs:103`);
  `PIN_WALL_MM` = 2.0 (`:95`); `place_keyed_pins` (`:360`).
- A native Pin Drill: a straight drill cycle, depth = stock height + spoilboard
  penetration (`compute/execute/drilling.rs:517`). The hole is the tool
  diameter; there is no helical bore.
- Z datum `MachineTable` (`session/mod.rs:784`): the export does not shift Z
  (`session/eval_context.rs:217`). XY is always stock-relative
  (`eval_context.rs:201-205`). The post `safe_z` is a fixed literal that the
  export does not shift, so it must be above the stock top (SAFEZ-LOCAL). The
  projects set `safe_z = T1 + SAFE_Z_CLEARANCE_MM` = 30.5.
- "After previous ops" stock across a setup boundary (FEATURE_CATALOG.md:84).
  A later setup is simulated on the stock that the earlier setup leaves.
- Project Curve From Below (`ops/project_curve.rs:164`, `:287`): on a Top
  setup it flips the mesh, drops the cutter and returns `z = surface + depth`.
  A negative depth puts the tip past the downward-facing surface.

It does not:

- Take a 3D polyline as a toolpath. The DXF import keeps X and Y only
  (`io/dxf_input.rs:196`, `polyline_to_points` at `:337`). So the V paths are
  2D lines projected on the terrain.
- Select by DXF layer. An operation takes every polygon of its model, so
  `derive.py` writes one DXF per operation. Nested closed rings become holes
  (`io/dxf_input.rs:159`).
- Project a point. Project Curve drops the real cutter. For a cut from the far
  side this is wrong where the surface is steeper than the cone (10° half
  angle): the cone touches a valley wall and the tip goes deeper. See check 1.
- Trace an open path as open. Trace closes every ring and skips a path with
  fewer than 3 points (`ops/trace_path.rs:164-178`). `derive.py` gives the
  2-point slit line a mid point; the closing move then runs back along it.
- Model a second material (epoxy) or a pour. The skim is simulated on the
  setup-1 result, with empty channels.
- Flip about Y. The test piece flips about its north-south line, so the job
  turns the piece 90° (see "Test piece").
- Report a per-setup or per-toolpath time. `rs_cam_cli project` prints one
  total; the per-setup and per-toolpath times here come from `nc-time` on the
  split program and agree with the simulation total to 0.3 %.
- Run this whole project in one CLI run inside 1200 s. A setup-2-only run
  takes 596 to 910 s of wall time for a 252-move skim, because its simulation first rebuilds the
  setup-1 stock. The checks therefore use one run per setup.

- Face the whole stock with `stock_offset` = r. The rows stop up to one
  stepover short of the far edge (see "Open items" 8).

Default findings that do not fit this job: `project_curve_negative_depth`
says a negative depth "cuts air". Here the negative depth is the overcut
past the front face, by intent.

## Frame and datum

- World frame = setup-1 frame = the back-view DXF frame of `block_back.dxf`:
  `x = 228 - X`, `y = Y + 313`, `z = T1 - Z` (block X, Y, Z). The numbers are
  `block.json` `block[2]` and `-block[1]`; `derive.py` asserts the map on all
  17 holes and all 19 616 V vertices of the CSVs (to 1.5e-3, the CSV rounding).
  The map is a rotation (determinant +1), so the STL winding stays valid.
- `T1` = 25.5 (operator): the faced back thickness. The stock is modelled as
  exactly T1 thick, `origin_z = 0`.
- Z datum `MachineTable` on every setup: Z0 = the spoilboard.
  - Setup 1 and 2 (Top) emit world Z: the back face is Z 25.5.
  - Setup 3 (Bottom) emits local Z = `T1 - z` = block Z: the back face on the
    spoilboard is Z 0, the raw front is 25.5, the faced front is 25.0.
- XY zero: the stock-local (0, 0) corner. The flip keeps it at the same
  machine point. It is (-5.0, -177.0) from pin 1.
- Smoke (one trace per side, `--emit-gcode`): setup 1 cut at Z 24.5 with the
  stock top at 25.5 (world Z); the flipped setup cut at local Z 24.5; the
  open test line moved from Y 28 to Y 326 = 354 - 28. The "origin_z must be
  negative" trap does not apply: with `origin_z = 0` the 2.5D ops start at
  the stock top (F-028).

## Stock, margins, pins

- Block 253 x 338. Margins: -X 16, +X 19, +-Y 8. Stock 288 x 354 x 25.5,
  origin (-16, -8, 0).
  - -X: `2 x PIN_WALL + pin 6 + outline tool 6` = 16.
  - +X: 16 + `PIN_KEYING_ASYMMETRY_MM` = 19, so the pin pair keys the flip.
  - Y: outline tool 6 + `PIN_WALL` = 8.
- Pins Ø6 (the 6 mm end mill drills them): stock-local (5, 177) and
  (280, 177); world (back frame) (-11, 169) and (264, 169). Keying margin
  3.0 mm (rs_cam gives no warning). Depth T1 + 4.5 = 30.0 = the flat_6
  stick-out. Wooden pins, no keep-out zone.
- CAUTION: the setup-3 face pass crosses the pins. Cut the pins below 25.0
  above the spoilboard.

## Setups and operations (nz-south)

Tools: `flat_6` (6 mm, 30 stick-out), `flat_3` (3 mm, 30), `v20` (20°,
3.175 body, 25), `v60` (1/4 in 60°, mech check.txt), the rivmap350 6.35 end
mill (front rough) and the R1.0 tapered ball (front finish).

Feeds: `feeds_oak.toml`. `flat_6` uses the `flat_6.35` row with depth and
width scaled by 6/6.35 (the row is for 6.35; rs_cam's depth gate refused
1.59 on a 6 mm tool). Full-width slots: half `feed_safe` (process_sheet rule).
V-bits and the tapered ball: the rivmap350 feeds (no row in feeds_oak).

Setup 1 "1 Back" (Top):

| # | Operation | Tool | From -> to (setup-1 Z) |
|---|---|---|---|
| 0 | Face back to T1 | flat_6 | 27.0 -> 25.5 |
| 1 | Pin drill, 2 pins | flat_6 | 25.5 -> -4.5 |
| 2 | Cavity rough D8.80 (pocket) | flat_6 | 25.5 -> 16.7 |
| 3-8 | Rough slot Z10..Z15 (pockets) | flat_3 | Z10 16.7 -> 15.5; Z11 16.7 -> 14.5; Z12..Z15 start at the floor before |
| 9-11 | Light holes Z15/16/17 (circle pockets, helix) | flat_3 | 16.7 -> 10.5 / 9.5 / 8.5 |
| 12-15 | V river, V lake: pre-pass and final (Project Curve, From Below) | v20 | tip = surface + 1.98 (pre) / surface - 2.2685 (final) |
| 16-25 | Mech pockets of `block_back.dxf` (10 layers) | flat_6 (ANTENNA_D5: flat_3) | from the back face or from the floor of the pocket they lie in |
| 26 | Mech THROUGH_D25.00 holes (Ø10, Ø8, helix) | flat_6 | 25.5 -> 0 |
| 27 | Day-strip slit V60 (Trace, 2 passes) | v60 | 4.83 -> -0.366 |

Setup 2 "2 Back skim" (Top, after the pour and the cure): op 28, Cavity
skim D9.00, flat_6, 16.7 -> 16.5, zigzag, one pass.

Setup 3 "3 Front" (Bottom, on the pins, fresh solid stock):

| # | Operation | Tool | Local Z |
|---|---|---|---|
| 29 | Face front to T | flat_6 | 25.5 -> 25.0 |
| 30 | Front 3D rough (adaptive3d, rivmap350 settings, leave 0.5, EDGE_OUTER inside) | 6.35 end mill | to 13.5 |
| 31 | Front scallop finish (rivmap350 settings, EDGE_OUTER inside) | R1.0 tapered ball | the surface |
| 32 | Outline profile, outside, 8 tabs (rs_cam default 6 x 2) | flat_6 | 25.0 -> 0 |

Decisions:

- The `block_back.dxf` cuts are in the job (setup 1, after the V-grooves):
  they are cuts in the oak block, and `check.txt` lists them under
  "block back". Excluded: `CAVITY_D9.00` (= the cavity layers),
  the `THROUGH_D25.00` outline (= the setup-3 profile), `PILOT_D12.00` (Ø2.5)
  and `PILOT_D20.50` (Ø2): no drill of that size is in `map.toml`.
- The day-strip slit is cut from the back (setup 1), not from the front:
  `block_back.dxf` gives it as `VGROOVE_V60_D25.866` from the slit slot
  floor at 20.67. From the front, a 1.0 mm slit at the face cannot reach the
  slot floor (4.33 mm of wood). With T1 = 25.5 the tip enters the spoilboard
  by 0.366; the setup-3 front face then makes the slit 1.0 mm wide.
- Rough-slot start depths. The slot layers do not nest exactly. `derive.py`
  (`slot_starts`) starts layer k at the floor of the deepest shallower layer
  j for which every part of (k - j) keeps the first-pass chip area at or under
  doc x woc (1.80 mm²). Result: Z10 and Z11 start at the cavity ceiling
  (Z11 has a 4.4 mm wide part outside Z10); Z12..Z15 start at the floor
  before (worst part 0.10 mm²). A union of the layers was tried: the
  tessellated union made cavalier panic 96 548 times and the run timed out.
- Mech pockets start at the floor of the pockets that contain them.
- Setup 2 uses fresh stock in generation (a 2.5D pocket from a pinned top
  does not read the stock); its simulation runs on the setup-1 result.
- Retract inside the cavity: slot, hole and V ops retract to the cavity
  ceiling + 5.0 = 21.7. The cavity is convex and every path is >= 4 mm inside
  it, so a straight rapid between two paths stays in it.
- Segment merge (tolerance = `[vgroove].tip_tol` 0.05) on the pockets. It
  halved the slot move count; the time is feed-bound and moved by 0.3 %.

## Deviations from the method, and why

| Deviation | Reason |
|---|---|
| The day-strip slit is cut from the back (setup 1), not the front | `block_back.dxf` gives it as `VGROOVE_V60_D25.866` from the slit slot floor (20.67); from the front a 1.0 mm slit cannot reach the slot |
| `block_back.dxf` pockets and the two control holes are in setup 1; `PILOT_*`, `CAVITY_D9.00` and the outline polyline are not | pockets: oak-block cuts; pilots: no drill in `map.toml`; the other two are the cavity layers and the setup-3 profile |
| Skim (setup 2) has `stock_source = fresh` | with "after previous ops" the generation ran a 15-minute setup-1 simulation first and the run timed out; a 2.5D pocket from a pinned top does not read the stock. The simulation still runs on the setup-1 result |
| Machine from rs_cam source presets, not from `rivmap350.toml` | `rivmap350.toml` holds "Generic Wood Router" with no kinematics; `shapeoko_vfd()` + `shapeoko_xxl_ricky_tuned()` hold the 1.5 kW spindle and the real `$$` |
| `flat_6` feeds = the `flat_6.35` row with depth and width scaled by 6/6.35 | no `flat_6` row; rs_cam's depth gate refused 1.59 on a 6 mm tool |
| Each V layer has a pre-pass (half the usable cone) before the final pass | a 20° bit full depth (up to 8.5 mm below the ceiling) in one pass is a heavy cut |
| Slot, hole and V retracts at 21.7, inside the cavity | the auto retract (30.5) doubles the air travel of 381 paths |
| Slot layers start at a computed depth, not all at the ceiling | the rule above (chip area); the layers do not nest exactly |

No change to rs_cam source. The job runs without core work; the core items
in "Open items" are improvements, with their measured cost.

## Checks

All on emitted motion, simulation resolution 0.25 mm (`map.toml [paths].grid`).

Runs: one `rs_cam_cli project --setup <name>` run per setup (a whole-project
run does not finish in 1200 s; see above). Setup 2 is simulated on the
setup-1 stock; setup 3 on fresh solid stock (the method). Peak RSS 4.7 GB.

1. **V accuracy** (`check_gcode.py` [1], V final passes against
   `vgroove_paths.csv`; emitted tip vs `T1 - depth`; `dz` > 0 = tip higher):

   | | vertices | XY vertex to path | tip dz p1 / p50 / p99 | dz min / max | within tip_tol 0.05 | line width p1 / p50 / p99 / max |
   |---|---|---|---|---|---|---|
   | river | 17 351 | p99 0.012, max 0.025 | -0.143 / 0.000 / +0.025 | -1.401 / +0.087 | 98.44 % (271 out) | 0.791 / 0.800 / 0.851 / 1.294 |
   | lake | 2 265 | p99 0.013, max 0.024 | 0 / 0 / 0 | 0 / 0 | 100 % | 0.800 everywhere |

   Result: FAIL against the bar "99 % within tip_tol". The lakes are exact
   (flat water). On the rivers, 271 vertices have a tip up to 1.40 mm deeper
   than planned, so the line there is up to 1.29 mm wide (0.8 nominal). Cause,
   measured on the 5 worst vertices: the V cone touches the valley wall of
   the flipped TIN before the tip reaches the floor (cone interference
   0.45..1.34 mm against dz 1.32..1.40). Project Curve drops the real cutter
   (a gouge guard); a cut from the far side needs a point projection. That is
   core work (see "Open items"). A no-core variant was tried: a ribbon mesh at
   the CSV tip Z as the projection surface. It is worse (64.9 % within 0.05,
   joints between ribbon quads take the higher contact), and it is not in the
   project.

2. **V-bit body** ([2]): at each emitted V point below the ceiling, the cone
   top (tip + 9.003) against the highest wood under the 3.175 body disc after
   the setup-1 roughing (slot floors from the DXF layers with the arcs
   flattened, else the cavity ceiling 16.7). Least margin: river final
   0.501 mm, lake final 0.534 mm, pre-passes 4.75 / 4.79 mm. No point is
   below 0; no point is under `body_margin` 0.5. PASS.

3. **Web over the cavity** ([3], setup-3 feed motion inside the cavity
   footprint + tool radius, local Z = block Z, floor = 9.0 + 4.0 = 13.0):
   face 25.000, rough 14.082, scallop finish 13.006. PASS (least margin
   0.006 mm, where the sea surface is at 13.0). The ball and the tapered ball
   touch with the tip first, so the tip Z is the tool contact.

4. **Simulation** (0.25 mm, tri-dexel, adaptive feed modulation on, the
   rs_cam default):

   | Setup | Toolpaths | Collisions | Rapid-through-stock | Sim time |
   |---|---|---|---|---|
   | 1 Back | 28 | 0 | 3 (Rough slot Z11: 2, Z12: 1) | 28 637 s |
   | 2 Back skim | 1 | 0 | 0 | 474 s |
   | 3 Front | 4 | 0 | 0 | 11 688 s |

   The 3 rapid flags are vertical G0 descents in the slot pockets (no rapid
   below Z 25.5 leaves the cavity: checked on the program). The CLI gives no
   position, so I did not locate them. Open item.

Also checked: the face passes now reach y 357 (tool centre) of the 354 stock
(before the fix: 350.4). The setup-3 motion lies where the flip puts it (relief ops
inside local x 37.5..247.5, y 29.5..239.5 less the tool radius; the outline
at x 13..272, y 5..349).

## Cycle times

Machine time from the simulation (F-034 integrator, `shapeoko_xxl_ricky_tuned`
kinematics). Per-setup numbers come from `--setup` runs (the CLI skip
mechanism); `nc-time` on the split grblHAL program agrees within 0.3 %.

| Setup | Modulated (rs_cam default) | Programmed feeds (`--no-adaptive-feed-modulation`) |
|---|---|---|
| 1 Back | 28 637 s (7.95 h) | 39 720 s (11.03 h) |
| 2 Back skim | 474 s (0.13 h) | 2 377 s (0.66 h) |
| 3 Front | 11 688 s (3.25 h) | 14 253 s (3.96 h) |
| Total nz-south | 40 799 s (11.33 h) | 56 350 s (15.65 h) |
| Test piece (3 setups) | 14 240 s (3.96 h) | 22 062 s (6.13 h) |

The modulation raises feeds up to the machine and load limits (the V passes
run up to 4 267 mm/min against the programmed 1 297); the programmed column
is the feeds_oak / rivmap350 numbers as written.

Largest items in setup 1 (modulated, `nc-time`): rough slots Z10..Z14
68 + 94 + 71 + 63 + 23 = 319 min; V river pre-pass + final 23 + 26 min.
Setup 3: front rough 50 min, scallop finish 126 min.

grblHAL program shape (master 66b03e0e): `M6 T<n>` at each tool change and
an `M0` with the message "M0 hold: no jog. To re-zero, export one file per
setup." between setups in a single-file export. With this method no re-zero
is necessary (one XY zero, Z0 = spoilboard on every setup), but export one
file per setup for the pour and the flip.

## Test piece

Source: `block/TESTPIECE.md` (= `build/block/testpiece/process_sheet.md`),
`block/testpiece.toml`, `build/block/testpiece/*`. Same method: T1 = 25.5,
raw top 27.0, MachineTable on every setup.

Mapping. The piece flips about its north-south line X = 0; rs_cam flips
about a line parallel to machine X. So the rs_cam world turns the piece 90°:
`x = Y + 60`, `y = X + 60`, `z = T1 - Z` (a rotation with the Z flip).
Put the piece on the machine with NORTH TO +X (right), not to the back. Then
the setup-3 raster rows run along local X = block Y (north-south), as the
sheet asks.

| rs_cam setup | TESTPIECE setup | Operations |
|---|---|---|
| 1 Back channels (Top) | 1 | Face back 27.0 -> 25.5; dowel bores Ø6 (flat_3 helix pocket, 25.5 -> -4.5); BACK_CHANNEL_Z17..Z20 |
| 2 Back cavity (Top) | 3 | Face the overfill 26.5 -> 25.5; BACK_CAVITY_Z9.00 |
| 3 Front (Bottom) | 4 | Face 25.5 -> 25.0; 3D rough (leave 0.5); parallel finish ball_3.18, stepover 0.25 |

- The dowel bores are a helical pocket, not the native Pin Drill: Pin Drill
  plunges the tool's own diameter, and the sheet bores Ø6 with the 3 mm end
  mill. The two dowels are still on the stock (`alignment_pins`) for the flip
  audit.
- rs_cam warns on load: "Alignment pins do not key the flip: the pattern is
  only 0.0 mm away from seating in the wrong orientation". The sheet's dowels
  (0, +-52.5) are symmetric. Monorepo item.
- Channel start depths: the same rule as the nz-south slots. Z18 starts at
  the Z17 floor and Z20 at the Z19 floor (worst part 0.10 mm²); Z17 and Z19
  start at the back face.
- Checks (`check_testpiece.py`): web over the cavity: rough 15.003, finish
  14.362 (floor 13.0; the sheet says the thinnest web is 5.5); channel motion
  inside the outlines less r (0 samples more than 0.05 mm out; the largest
  excursion is 0.034 mm). Simulation: 0 collisions, 0 rapid flags.
- Times (modulated, `nc-time`): setup 1 123 min, setup 2 13 min, setup 3 103 min,
  total 14 240 s (3.96 h) with modulation, 22 062 s (6.13 h) at the
  programmed feeds.

## Open items

Needs core work (rs_cam):

1. **Point projection for Project Curve** (or a 3D-polyline toolpath). The
   V tips are wrong on 1.6 % of the river vertices (up to 1.40 mm too deep,
   line up to 1.29 mm wide) because the drop cutter treats the far-side
   surface as a surface to protect. A `projection = point` option on
   `ProjectCurveConfig`, or a DXF import that keeps Z for an open 3D
   polyline, removes the error.
2. **Trace of an open path** closes it and skips fewer than 3 points
   (`ops/trace_path.rs:164`). Harmless for the straight slit; wrong for any
   open curve.
3. **Setup-2 simulation cost**: a one-op setup rebuilds the earlier setups'
   stock (596 to 910 s for 252 moves), so the whole project does not fit one 1200 s
   CLI run.
4. **Rapid flags without position** in the CLI output (3 in setup 1).

Not core (operator or lead):

5. Feeds for `v20`, `v60` and the R1.0 tapered ball are the rivmap350 values;
   `flat_6` is the scaled 6.35 row. Adaptive feed modulation (the rs_cam
   default) raises the V feed to 4 267 mm/min; the programmed V feed is 1 297.
6. `v60` stick-out is assumed 25 mm; measure it. Measure the `v20` body.
7. Tabs: 8, width 6, height 2 (rs_cam defaults); confirm.
8. Fixed in the job: with `stock_offset` = r the face rows stopped at
   y 350.4 of 354 (`ops/face.rs:142` builds the rectangle, the zigzag rows
   step from the inset edge and add no row at the far edge), so a 0.6 mm
   raw strip stayed on the back at the stock edge. In setup 3 the back lies
   on the spoilboard, so that strip (up to 1.5 mm high) would lift the stock.
   The faces now use `stock_offset` = the tool diameter.
9. The pilots (Ø2.5 x 12, Ø2 x 20.5) are not in the job.
10. The slot feed is half `feed_safe` (541.5 mm/min) on every pass. The slot
    layers are merged pockets (the largest is 16 356 mm²), not 4 mm slots, so
    most passes are not full width. The slots take 5.3 h of the 7.9 h of
    setup 1 (modulated).
11. The 3 rapid flags in setup 1 (look at them in the GUI before a cut).
12. Every V path starts with a straight plunge at 730 mm/min, up to 8.5 mm
    into oak (381 + 24 paths; Project Curve has no ramp). Operator judgement.
13. The pin drill is the 6 mm end mill, plunge-drilling 30 mm in 6 mm pecks.
    Operator judgement (a drill or a helix is gentler).
14. CAUTION: cut the wooden pins below the setup-3 face height (25.0) or the
    face pass cuts them.

## The list for the monorepo

The block stage writes most of what the job needs. These items close the
gaps. Formats are exact; units mm; frame = the back-view frame of
`block_back.dxf` unless the line says otherwise.

1. **`block.json` → a `cam` object** with the stock and the facing:

   ```json
   "cam": {
     "T1": 25.5,              // faced back thickness = stock height in rs_cam
     "raw_top_max": 27.0,     // highest raw stock top the back face pass starts from
     "face_front": 0.5,       // T1 - thickness, removed by the setup-3 face pass
     "stock_margin": {"minus_x": 16.0, "plus_x": 19.0, "y": 8.0},
     "pins": [{"x": -11.0, "y": 169.0, "d": 6.0}, {"x": 264.0, "y": 169.0, "d": 6.0}],
     "pin_depth": 30.0        // T1 + spoilboard 4.5 = the flat_6 stick-out
   }
   ```

   The margins follow the rs_cam pin rule (wall 2 + pin + wall 2 + outline
   tool; +3 on +X to key the flip). The pins are on the flip axis
   y = 169 (= the block centre line), in the X margins.
2. **One DXF per operation, or a layer manifest.** rs_cam cuts every polygon
   of a model. Either keep `back_channels.dxf` and add
   `cam_ops.json`:

   ```json
   [{"layer": "ROUGH_SLOT_Z12.00", "op": "pocket", "tool": "flat_3",
     "z_start": 11.0, "z_end": 12.0, "entry": "ramp"}, ...]
   ```

   with `z_start` = the block depth where the material of that layer starts,
   or write the per-layer files that `derive.py` writes now.
3. **Nested slot layers.** Make each `ROUGH_SLOT_Z<k>` lie inside
   `ROUGH_SLOT_Z<k-1>` (or give `z_start` above). Today 59 mm² of Z11 lies
   outside Z10, one part 4.4 mm wide, so Z11 must start at the cavity ceiling
   and recut 1.2 mm of air over 15 900 mm².
4. **The terrain in the setup-1 frame** (optional; `derive.py` does it):
   `terrain_back.stl`, binary, the closed solid of `terrain.stl` with
   `x = 228 - X`, `y = Y + 313`, `z = T1 - Z`. The map is a rotation; keep the
   triangle order.
5. **The V paths.** rs_cam reads only the 2D line of `VGROOVE_V20_*` and
   projects it on the terrain with the tip offset
   `(line_width / 2) / tan(vgroove_angle / 2)` = 2.2685 past the surface.
   Keep `line_width` and `vgroove_angle` in `block.json` and keep
   `vgroove_paths.csv` (the check reads it). No new file is needed until
   rs_cam can take the 3D polylines.
6. **The flip convention.** rs_cam turns the stock 180° about a line parallel
   to machine X through the stock centre (`FaceUp::Bottom`). In the block
   frame that is a flip about the block's east-west axis: north and south
   change places, east and west stay. The block stage's back view mirrors X
   (`x = 228 - X`); with the flip about X, the setup-3 frame is the block
   front view turned 180° (`x = 244 - X`, `y = 33 - Y`).
7. **Test piece flip axis.** The test piece flips about its north-south line.
   Either turn its frame by 90° in the stage (dowels on a line parallel to
   block X), or keep it and the operator turns the piece (the job does this).
   Also move one dowel by >= 3.0 mm along the axis, so the pair keys the
   flip (now symmetric at (0, +-52.5)).
8. **Through cuts at T1.** `THROUGH_D25.00` must cut to T1 (the stock
   bottom), not 25.0. `VGROOVE_V60_D25.866` then enters the spoilboard by
   25.866 - T1 = 0.366.
9. **Tools and feeds.** Add `v60` (60°, 6.35, stick-out) to `map.toml`;
   add rows for `flat_6`, `v20`, `v60` and the front finish tool to
   `feeds_oak.toml`; add a drill for `PILOT_D12.00` (Ø2.5) and
   `PILOT_D20.50` (Ø2) or mark them "drill by hand".
10. **One polygon per pocket.** `BOARD_POCKET_D14.50` and `WIRE_D5.00` hold
    two overlapping polygons each; rs_cam pockets each one, so the overlap is
    cut twice. Write the union.
