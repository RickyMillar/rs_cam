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

## Update 2026-10-09

- Open item 1 is FIXED: Project Curve has `projection = "point"` (1b91b995). The four
  V ops of nz-south use it (`derive.py`, `nz_south.toml`). Acceptance on the emitted
  grblHAL G-code: river 17,351 and lake 2,265 vertices 100 % within 0.05 mm (was
  98.44 %), line width 0.7998-0.8002 mm.
- `testpiece.toml` now holds operator edits made in the GUI on 2026-10-09:
  - the grblHAL machine import;
  - a 6 mm twist-drill peck op for the dowels (the helix bore is disabled);
  - feeds from Suggest (scope both; the finish stepover kept at 0.25).
  A run of `derive.py` overwrites these edits; carry them into the script before the
  next run.

## Update 2026-10-11: the mini test piece

The operator's stock is 25 mm thick and small: "can we make it only one section, and make
the whole thing smaller of a test". The mini piece has one group (B, not sealed, clear) and
one row (margin 1.0) on 65 x 65 x 25 stock.

- Block config: `testpiece_mini_block.toml` (a copy of the monorepo `block/testpiece.toml`;
  its header says what changed and how to run the generator). The monorepo generator writes
  to `/home/ricky/cnc_jobs/rivmap_block/testpiece_mini_src/`, outside the monorepo. All its
  checks pass (web 9.70, dowels 3.00 to the cavity and the edge, reach 23 of 30).
- Method: raw top 25.0, T1 = 24.5, T = 24.0. Channels `BACK_CHANNEL_Z22.00` and `Z23.00`;
  the channel floor is 1.5 mm over the spoilboard.
- Project: `derive.py testpiece-mini` writes `testpiece_mini.toml` with the operator's edits
  (grblHAL controller, the 6 mm drill, the pour as a stock change at T1 + 1.0). Then Suggest
  (scope both; the finish stepover kept at 0.25) ran in the GUI, and the GUI saved the file.
- Simulation: 0 collisions, 0 rapid collisions; pour 10.48 ml measured (sheet: 10.0 + 0.4
  overfill); 3 948 s total. The dowels are symmetric on the flip axis: the rim has no room
  for the 3 mm keying offset. Mark one edge of the part.
- G-code: `~/Downloads/testpiece_mini_gcode/` (3 files, the flip-message fix 5b428b53).

## Update 2026-10-31: the full-size block nz-wanaka-21

Brief: `project_rivmap_mono/manufacture/docs/rs_cam_full_block_prompt.md`. The operator asked for
the setups, the tools and the toolpaths only; the operator tunes them in the GUI. No simulation,
no checks and no G-code are done yet.

- `derive_wanaka21.py` writes `nz_wanaka_21.toml` (3 setups, 33 toolpaths, 6 tools). It reads
  `block.json` `cam` and `cam_ops.json` (op order, tools, z_start, z_end; the "hand" rows are
  skipped) and reuses the writers of `derive.py`. It removes the keys that the loader now retires
  (toolpath `type`, `fixtures`, `keep_out_zones`, `air_bridge_policy`, `shank_length`) and writes
  the EDGE_OUTER boundary as a selection. `derive.py` (nz-south, test piece) is not changed and
  no longer runs: its constant asserts name a removed constant.
- Stock: the operator's 395 x 460, the block centred (18.5 / 8.5). Machine: the Scylla grblHAL
  tables of `testpiece_mini.toml`. Pins: rs_cam preset "axis plus mirror pair", 3 x Ø6, centred
  in the band between the stock edge wall and the profile cut wall (6.25 mm from the long edge);
  the pair is 100 mm from the flip axis (a free choice). rs_cam gives no "not keyed" finding.
- The V ops take top Z = the cavity ceiling (the loader refuses a retract below the top).
- The skim uses fresh stock (a remaining-stock skim waits for a setup-1 simulation).
- The pour is a stock change on setup 2: the cavity outline, filled to ceiling + overfill = 17.7.
- All 33 toolpaths generate (GUI at fa6da4fed). Findings: the scallop plunge 736 is over the
  tapered ball cap 300 (run Suggest); the v20 name "25 mm stick-out" reads as Ø25 (info).

### Revision 2026-10-31 (later): 10 toolpaths with 3D ops

Operator: "use 3d ops for most of this, and use maybe like.. 5-10 paths total". The back is now a
3D rough of the finished oak block (mech `oak_block_full.stl`, moved to the setup-1 frame with
`stack.json` levels.z_block) with flat_6, then a flat_3 rest rough from remaining stock; the V
lines are one Project Curve op (no pre-pass); the cavity is cut to its finished depth 9.0 and the
skim takes only the overfill. The V60 day-strip slit is not in the job (a 3D rough cannot cut a
1 mm slit). All 10 generate; the project simulation (0.25 mm) gives 10 h 41 min, 1 rapid
collision on the front 3D rough (z 23.086, run move 250349), the scallop plunge finding, and
2 of 10 operations over a load gate.

### Revision 2026-10-31 (evening): hollow under the relief, 13 toolpaths

Operator: rough the relief from behind (the earlier rivmap350 method), drill the light holes,
cut the day-strip slit from the back before the same pour.

- "Back hollow under the relief 6 mm": adaptive3d on the front-surface triangles of
  `terrain_back.stl` (a sheet; the drop cutter meets the relief, not the flat back), inside the
  cavity, from the cavity floor down, leave = `web_min` 4.0. The V then cuts at most
  4.0 + 2.27 = 6.27 of its usable cone 8.50, so no rough slots; the rest (2.2) caps the depth per
  pass. A 4.8 stepover (feeds_oak) gave 957 m of feed and 4.6 km of rapids (40.6 h total);
  the adaptive stepover 1.5 gave 595 m and 233 m (16.7 h total, 0 collisions, 2 rapid flags).
- Light holes: one Drill op, the operator's 6 mm twist drill, all 42 holes to the deepest layer
  (block z 17) with the full diameter; size_tolerance 0.2 takes 6.0 against the drawn 5.81.
- ANTENNA_D5.00 (4 mm wide) is a 3 mm 2D pocket; every other mech pocket fits the 6 mm.
- Day-strip slit: V60 trace in setup 1; a second outline fill of the pour fills it to the slit
  slot floor. CAUTION: the slit is open under the stock (tip 0.366 into the spoilboard).
- Open: rapid flag on the front 3D rough (z 23.086); the scallop plunge (736 > 300);
  Project Curve has no open-path offset and no depth per pass (core work, not started).
- Performance (operator: "definitely struggling"): the models were 2.7 M triangles (oak_block_full
  1.4 M for the back rough, terrain_back and the relief sheet 0.65 M each). The back rough now
  reads mech `block_back.stl` (8 k triangles; same features: 162.8 m against 166.3 m of feed),
  and one relief sheet serves the hollow, the V projection and the front ops: 0.66 M in total.
  The V point spacing is map.toml `[vgroove].max_seg` 0.25 (was tip_tol 0.05): 21.9 k moves
  (was 106 k). Total 732 k moves, of which the scallop finish is 562 k. 16.5 h, 0 collisions.

### Revision 2026-10-31 (night): one back rough, lakes, deeper rivers, geodesic, Suggest speeds

- One back rough (flat_6, geodesic) on `back_target_s1.stl`: the mech block back without its
  triangles inside the cavity, plus the relief sheet moved web_min toward the back (the hollow),
  plus the sheet moved `exposure_margin` (contract, 1.0) past the front inside the 9 closed lake
  outlines (the lakes open at the front; the 2 open lake paths stay V-only). ANTENNA_D5 stays a
  3 mm pocket (4 mm wide).
- The V tip goes overcut + exposure_margin = 3.27 past the front (line about 1.15 mm, was 0.8);
  the depth per pass of the back rough is the rest of the usable cone, 1.2.
- Suggest (scope speeds) on 11 of 12 ops in the GUI, then the GUI saved the file. Suggest refuses
  the drill (a twist drill is modelled as a V-bit: no G6 basis); it keeps the operator's
  600 mm/min, 6000 rpm. CAUTION: a derive_wanaka21.py run writes the feeds_oak speeds again.
- Simulation 0.25 mm: 10.47 h, 0 collisions, 0 rapid collisions. Back rough 270 k moves,
  1186 m of feed; one depth-of-cut caution 13.3 mm (over the 2 x D rule) on it.
- Lakes 3 mm (operator: "the lakes still have some un milled areas"): the 6 mm reaches a lake
  only where its disc fits (the target inside a lake is about 5 mm below the hollow around it).
  A flat_3 adaptive3d (contour parallel) on the lake surface alone, inside the closed lake
  outlines, from the cavity floor: 27 k moves, 74 m. The sim (checkpoint after the op) shows the
  lakes cleared to their floor, arms included; the lake island stays. Total 12.27 h, 0
  collisions, 0 rapid collisions. Suggest speeds applied again (the drill keeps its own).

### Revision 2026-11-01: roughing time, measured

The back (setup 1) was 9.38 h of the 12.27 h (measured on setup 1 alone); the front is 2.71 h.
The finish was never 6 h (an earlier reading of the GUI timeline bar was wrong).

| Variant (whole job, 0.25 mm sim) | Back paths, m of feed | Total |
|---|---|---|
| 6 mm back rough alone (1.2 x 1.5 mm) | 1186 | 12.27 h |
| 18 mm bulk 2 x 10.5 mm, then 6 mm rest (remaining stock) | 49 + 230 | 6.89 h |
| 18 mm bulk 3.0 mm steps, then 6 mm rest | 127 + 226 | 7.42 h |
| 18 mm 3.0 mm, R3 ball scallop over the hollow, 6 mm rest | 127 + 49 + 110 | 7.66 h |
| Tiered finish R3/R2/R1 instead of the R1 scallop (front only) | 468 + 519 rapid | front 15.2 h against 2.71 h |

- The project takes the 6.89 h row. Bulk levels: top to the deepest hollow point in the fewest even
  passes inside the 20 mm flute, no flat-area levels (they added five 0.1-0.7 mm passes).
- The 6 mm rest walks 17 levels of 1.2 mm (63 % air): the next lever.
- The operator's bits are in `~/.config/rs_cam/tools/my_roughing.toml` (stick-outs: measure).
- The skim is removed (operator); setup 2 holds only the pour. The front 3D rough and scallop keep
  the operator's GUI values.
- Defects found: the GUI memory budget reads half of RAM and ignores the proxy's 16 GB cap (an
  OOM kill); the tier planner has no "inside a boundary" mode; a lake-pass rapid flag at Z 8.0.
- Tool order and the front (operator 2026-11-01): the 18 mm bit faces both sides (stepover 0.8 x D
  = 14.4) and roughs the front bulk (3 even levels over the 12.5 mm relief, leave 0.5); the 6 mm
  front rough is now its rest. The pins and the light holes follow the 6 mm back rest (one tool;
  the holes are a Drill op with the 6 mm end mill, flat bottom, G6 feeds; the twist drill is gone).
  Back tool changes 6 -> 4. Whole job 6.89 h -> 6.64 h, 0 collisions, 0 rapid collisions.
  Left: the 6 mm back rest (230 m, 62 % air), the scallop finish (accel-bound), the 3 mm lake
  pass (146 m of rapids).

### Overnight 2026-11-01/02: iterations (whole job, 0.25 mm sim)

| Experiment | Result | Kept |
|---|---|---|
| E1: 18 mm 3 mm levels, R3 ball over the hollow, 6 mm rest at Suggest depth 4.69 | 7.02 h (ball 49 m at 1174 mm/min; bulk 127 m) | no |
| E2: 6 mm rest contour-parallel instead of geodesic | 10.30 h (434 m feed, 846 m rapids) | no |
| E3: segment merge 0.02 on the front scallop (under its 0.05 tolerance) | 6.53 h (562 k -> 466 k moves) | yes |
| SAFETY: the 3D roughs merged at the default 0.3, which can cut 0.3 below the target and thin web_min | merge tolerance = tip_tol 0.05 on every rough: 6.71 h | yes |

The committed project is the last row: 6.71 h, 0 collisions, 0 rapid collisions.

### Overnight 2026-11-02: STATUS — simulates clean, does NOT export

CAUTION: the committed project (e84dd837e) simulates with 0 collisions and 0 rapid collisions, but
`list_toolpaths` shows "Back bulk rough 18 mm" and "Back rest 6 mm" blocked by
`safety.cuts_into_model`: export refuses while it stands. Most likely every version since the 18 mm
bulk pass (932fd7538) has it; the per-op run states were not read until now.

- The gouge: 2-4 of 30 k-340 k sampled cutting positions go 0.026-0.049 mm past the limit
  (target surface + leave - op tolerance), so about 0.09-0.13 mm below the target, between the Z
  levels (bulk: (287.9, 187.7, Z 11.57); rest: (232.4, 337.6, Z 15.22)). In the hollow that is a
  local web of about 3.9 mm (web_min 4.0). Tried: segment merge off -> no change (merge was not the
  cause; the e84dd837e merge note is wrong on the cause); op tolerance 0.1 -> 0.05 -> overshoot
  0.026 -> 0.044 (the limit rises with it). The finding names it a planner defect. Operator
  choices: report and fix the adaptive planner (core), or accept it (a larger op tolerance passes
  the gate and keeps the ~0.1 mm dip).
- Entry load, critical, on both 6 mm rests: the back rest's entries cut up to 14.5 mm of standing
  wood at (211.7, 41.7, Z 11.0) (board pocket; the 18 mm levels at Z 15 / 4.5 leave it), and links
  cross up to 13.05 mm (2.7 % of samples over the 1.2 commanded); the front 6 mm rough's entries
  up to 4.57 mm at (289.4, 338.7, Z 20.36). Candidates: helix entry; a bulk level at the pocket
  floors.
- Front scallop: crosses material the rough left, 12.4 % of samples over 1.26 mm, peak 6.09 mm
  (R1 tapered ball). A rest pass before the finish would carry that load.
- Day strip slit: 49.5 % of samples bite more than the commanded 2.6 mm, peak 5.27 mm. Open
  question: is the 7 mm slit slot floor (Z 4.83) cut by the 6 mm rest before the V60? Check the
  sim checkpoint after the rest at the slit.
- rs_cam defects: `generate_all` reported `not_ready: []` while two ops were blocked; the
  project verdicts do not carry per-op blocking or critical findings (only `list_toolpaths` /
  per-op diagnostics show them).

### 2026-11-02: the gouge is fixed (56f3381e7, G-DRAPEGAP)

Cause: the 3D Rough planner draped each cut at one tool radius and assumed no peak hides between
two samples; a point beside the path does. Fix in `adaptive3d` (the chord lift, with the guard's
reduced tool). The job regenerated on the fixed build: both back roughs are no longer blocked; 6.72 h
(+20 s), 0 collisions, 0 rapid collisions. Still open (unchanged): the entry-load criticals of both
6 mm rests (back 14.5 mm at (211.7, 41.7); front 4.57 mm), the scallop crossing (peak 6.09 mm), the
slit (peak 5.27 mm). A synthetic or cropped integration fixture did not reproduce the dip (the planner
places passes differently); the sentry is the lib test plus this job.

### 2026-11-02: operator tools, the 2 mm lake rest, the engraving

Changes (operator 2026-11-02):

- Tools: the DMM DSP1f-D2.0-L08 (2 mm, 1 flute, 8 mm flute; shank 3.175 assumed: measure) replaces
  the 3 mm on the lakes and the ANTENNA_D5 pocket (one tool change less). The V20 is the
  3.175-20deg-0.3: rs_cam models a sharp tip, so the generator puts the 0.3 mm flat tip into the
  depth (tip = (line - 0.3) / (2 tan 10 deg) past the surface) and the cone ((3.175 - 0.3) / 2 / tan 10
  deg = 8.15). The finish ball is the R1 x 6 x 20 mm 2F; its taper half angle 5.711 deg is from that
  size (atan(2 / 20)), not a printed value: measure. The 6 mm flat has a 25 mm cutting length.
- "Lakes 2 mm (rest)": remaining stock, after the 6 mm rest, so it cuts only what the 6 mm left.
- "Engrave day labels V60": cam_ops.json ENGRAVE_V60_D24.40 (rebuilt 2026-11-01), trace, block z
  25.0 -> 24.4 in one pass, after the front face, before the outline profile. 41 of the 49 strokes
  have 2 points; the generator adds the mid point (Trace skips a path under 3 points).
- Front 3D rough: the EDGE_OUTER boundary with the tool centre on it (was tool inside).

Result (whole job, 0.2 mm sim: the 2 mm tool needs a cell of tip radius / 5): 7.21 h (was 6.72 h),
0 collisions, 0 rapid collisions, no op blocked.

| Op | Time | Note |
|---|---|---|
| Lakes 2 mm (rest) | 2.21 h | cutting 333 s, entries 3607 s, rapids 3793 s (172 m) |
| Back rest 6 mm | 1.76 h | |
| Front scallop finish | 1.60 h | |
| Front 3D rough | 0.50 h | |

Findings:

- The lake pass is now the largest single op, and it is almost all entries and rapids: remaining
  stock breaks the lakes into thousands of small leftovers, each with its own ramp entry at the 2 mm
  plunge feed. Critical `project.entry_load`: peak 4.67 mm at (124.8, 208.5, Z 7.74).
- The front scallop still crosses standing material (13.7 % of samples over 1.08 mm, peak 5.39
  mm; was 12.4 % / 6.09 mm). The centre boundary did not remove it. The relief has narrow valleys
  that a 6 mm flat cannot reach to the leave; the R1 ball takes that material. A rest pass with a
  smaller ball before the finish would carry it.
- Back rest entry load: still 14.5 mm at (211.7, 41.6, Z 11.0). The back target there is a
  pocket floor at Z 11.0 beside a wall up to Z 25.5. The bulk levels (Z 15.0, 4.5) do not reach
  the floor. Not resolved.
- Front 3D rough entry load: 4.62 mm at (168.9, 338.9, Z 20.38). Not resolved.
- Note: `board_pocket_d14.50.dxf` and other DXF files in the job folder are from an older job;
  the current back target does not match their outlines.

### 2026-11-02 (later): the lakes as flat-floor 2.5D pockets

The remaining-stock lake pass was 2.21 h, almost all entries and rapids. A lake is water, so its
surface is flat: inside one outline the sheet varies at most 0.57 mm (15 closed lakes measured).
The generator now writes 2.5D pockets with the 2 mm tool:

- Floor of a lake = its lowest sheet point (triangles inside the outline; the band around it holds
  the river that leaves the lake, which runs lower) - 2 x exposure_margin. The target is one margin
  past the front; the second margin lets the pour go deeper, so dust or bubbles at the pour boundary
  stay out of the visible edge (operator 2026-11-02). All of it is front waste.
- The walls are vertical: the front shows the outline at any depth. One finishing contour.
- Lakes share a pocket at the floor of their deepest member while every wall (shore hollow - floor)
  stays within the 8 mm flute less map.toml body_margin (7.5). Result: 8 lakes to Z 5.91, 1 lake to
  Z 3.99. Lakes narrower than the tool (6 of 15) stay V-only.
- Top = the shore hollow + one rest step (the 6 mm rest leaves at most that above the hollow).
- Result (0.2 mm sim): whole job 5.81 h (was 7.21 h with the remaining-stock lake pass), 0
  collisions, 0 rapid collisions, no op blocked. The lake entry-load critical is gone.
- rs_cam defect (G-POCKETRAMP): a Pocket with the ramp entry dressup made 21 rapid collisions in the
  large lakes: at two ring starts each level's rapid descent ends on the floor of the level before,
  and wood stood there. The feed plane did not change it (the Pocket ignores `feed_z`), the region
  order did not change it; a plain plunge entry removes all 21. The lake pockets use the plunge entry.
- Still open: the entry-load criticals of the back rest (14.5 mm, (211.7, 41.6)) and the front 6 mm
  rough (4.62 mm); the front scallop crossing (13.7 %, peak 5.39 mm: narrow valleys a 6 mm flat
  cannot reach); the slit (49.5 % over 2.6 mm).
- Next (operator 2026-11-02): top clamps at y 153 and 307 on both x sides (the thirds; symmetric
  for the flip; 10 mm clear of the right pins at 130 / 330). Keep-out about x 0-10 from the edge,
  y 135-171 and 289-325. Add them as obstacles in setups 1 and 3, and a "Face clamp pads" op after
  the pin drill (pins in, top clamps off), so the flipped back sits flat. Waiting for the measured
  positions and the clamp height.

### 2026-11-02 (evening): queue and operator inputs

Operator inputs: stock thickness 26 mm; top clamps 4 mm tall, 20 mm wide, keep-out 40 mm along y,
placed at y 153 and 307 on both x sides; spindle 1.5 kW.

Findings (test export to the scratchpad with overrides, not for cutting):
- The 18 mm bulk bit does not cut at the centre (core about 8 mm, blades about 1 mm proud). The back
  bulk emits 89 straight G1 plunges of more than 3 mm into wood (for example Z 26 -> 6.31 at (212.7,
  59.0)); the front bulk has 175 descents over 1 mm steeper than atan(1/8) = 7.13 deg. Do NOT run the
  18 mm ops until rs_cam models the tool: prompt in `planning/centre_cutting_tool_prompt_2026-11-02.md`
  (G-CENTRECUT, sent to another agent).
- Back bulk exceeds spindle power (1.18 kW peak, 1.01 kW available at 16 219 rpm on the power curve).
- The in-cavity retract (Z 21.5) of the V op and the lake pockets: 637 G-code validator errors.
- The project stock is 25.5 thick (= T1): the back face cuts only air in the sim.
- The deep 6 mm wall bites: 3D Rough skips regions shorter than `min_region_cut_length_mm` (15 mm);
  the corner slivers the 18 mm leaves are shorter at every upper level, so the rest takes them in one
  bite at the floor.

Queue: stock 26 mm; clamp obstacles + "Face clamp pads"; min_region_cut_length below the 9.4 mm
corner arc on the 6 mm rests; R1 tapered rest before the scallop (leave 0.5); bulk feed for the power
cap; retract at clearance; the back hollow only as deep as the rivers need (operator: from the back
the front valleys are high and the mountains deep; a hollow under the mountains does no work — rough
less and trace the rivers with the 6 mm first, as in the earlier jobs).

### 2026-11-02 (night): queue results

Added: top clamps as obstacles in setups 1 and 3 (4 boxes, 40 mm along the edge, 10 mm onto the
stock, top = raw top + 4, clearance 1); five pins, 3 left (y 190 / 230 / 270) and 2 right (y 110 /
350), symmetric about the flip axis and asserted clear of the keep-outs; setup "1b Clamp pads" (pins
in, top clamps off, 6 mm pockets the pads from the measured raw top 26 to T1); rapids of the V op and
the lake pockets at the clearance plane. The model stock stays T1 thick (setup 3 zeroes on the faced
back), so the raw 0.5 mm skin is not modelled.

Measured (0.2 mm; the full sim needs 26.8 GiB, so the scallop was off for the measuring runs):

| Variant | Without the scallop | Note |
|---|---|---|
| Channel hollow + R1 whole-relief rest + min_region 0 | 47.9 h | R1 rest 4266 m cut / 3248 m rapids; back rest 537 m |
| Channel hollow, no R1 rest, min_region 15 | 5.39 h (about 7.0 h with the scallop) | back rest 284 m / 144 m rapids (was 232 / 63) |

0 collisions and 0 rapid collisions in both. Kept out of the generator: the R1 rest (no "only where
the 6 mm could not reach" region for a 3D Rough) and min_region 0 (more regions, no fix: the 14.5 mm
entry at (211.8, 42.0, Z 11) stays even with a bulk level at Z 11). The channel hollow is behind
`HOLLOW_CHANNEL_ONLY = False`: it pays only with a 6 mm trace along the rivers (channel 0-2.5 mm below
the cavity floor in the valleys, 4.5-5.5 near the lakes), which needs Project Curve depth per pass.
The GUI ran OOM at the 16 GB cap; the proxy cap is now 28 GB.

### 2026-11-02 (late): new rs_cam features in the job

- rs_cam c0f5d2567: G-CENTRECUT part 2 (entries limited, export refuses a descent into stock past
  the limit) and the 3D Rough rest region filter. The 18 mm bit now states core 8 / protrusion 1
  (operator, "around": measure). GUI rebuilt from that commit.
- Hollow mode "trace" (default): the back target keeps a hollow band only around the lakes; the
  river channels are ONE 6 mm Project Curve pass along the V20 lines, cutter projection from above,
  tip = the highest sheet point under the disc + web_min, at the feeds_oak flat_6 row (826 mm/min,
  10 610 rpm). Suggest does not know that this Project Curve is a full-width slot.
- R1 valleys rest with `rest_max_region_width_mm = 6`: 301 m cut, 676 m rapids (was 4266 / 3248
  without the filter). Removed: the deep scallop bites are the 6 mm rough's terraces on every slope
  (front depth per pass 5.53, the operator's value), and those slivers are narrow too. Next lever: a
  smaller front 6 mm depth per pass.
- Not yet measured as a whole: one run stopped early (the back rest stayed Pending while the
  operator edited in the GUI), so its 7.3 h (no scallop, no back rest) is not a result.
- Open in rs_cam: every_op_keeps_its_toolpath_g_opgolden (8 move hashes; golden from 10-28) and
  every_op_conforms_w4 check6; the front 3D rough planned for about 10 min on the new build (about
  2 min before): check for a slowdown.
- Machine rigidity (operator 2026-11-02, "industry standard, a bit softer"): in the machine library
  `~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml` woc_roughing_factor 0.8 -> 0.4 (the soft
  end of rs_cam's own pocket guidance, 40-60 % of D) and adaptive_woc_factor 0.25 -> 0.2. The
  generator now reads the rigidity factors from that file. Lake and antenna pockets step at the tool
  radius (a 1.6 mm step on the 2 mm tool left a centre sliver).
- rs_cam defect candidate (G-R1HANG): the operator's R1 rest variant planned for 4 h without an end
  (cancelled): 3D Rough, R1 x 6 x 20 tapered ball, remaining stock, geodesic_adaptive, region_ordering
  by_area, detect_flat_areas true, rest_max_region_width_mm 20, stepover 0.66, depth_per_pass 2.0,
  leave 0.5, bottom_z 13.0, boundary EDGE_OUTER centre.
  Second defect: cancel did not stop it (lane "cancelling" for about 50 min after the cancel at
  14 466 s): that 3D Rough loop does not poll the cancel flag. The GUI was restarted (SIGTERM).
