# Lead proposal for the wanaka job (2026-11-15)

File: `nz_wanaka_21_lead_proposal.toml`. It is a copy of `nz_wanaka_21_operator.toml` with the
changes below. The operator file is not changed. The operator decides if the proposal replaces it.

Measured on the release GUI of `28305b389`, clean load, one Generate All, one simulation at
0.2 mm, feed modulation cap only. Scripts and exports: the lead scratchpad `job/`
(`band_thickness.py`, `skin.py`, `add_band.py`, `round2.py`, `proposal_round2.nc`).

## Changes

1. **"River channel 6 mm, pass 7 of 7": `depth_per_pass = 2.1`, top Z 25.5.** The passes are
   inside the one toolpath (operator ruling 2026-11-15). 2.1 mm gives 7 passes over the 14.3 mm
   below the cavity floor, as the script `derive_wanaka21.py` made them, and is below the advisory
   force-budget depth of 2.70 mm.
2. **Three new Profile toolpaths before the 2 mm lake pockets**, 6 mm tool, side `on` (the tool
   centre on the lake edge, so the band reaches 3 mm outside the lake), feed 3165 mm/min at
   10610 rpm (the river values), `depth_per_pass = 2.1`, top Z 25.5, no feed optimisation:
   - "Lake relief band 6 mm, 10 lakes to Z 13.5" (model 15, all rings but one);
   - "Lake relief band 6 mm, lake at X325 Y220 to Z 14.9" (model 15, one ring);
   - "Lake relief band 6 mm, 1 lake to Z 11.4" (model 16).

## Why the band is 3 mm wide and not 6 mm

The job design keeps about 4 mm of wood between the back cavity and the front relief
(`hollow_s1.stl` against `relief_sheet_s1.stl`: first percentile 3.78 mm; the river depth is
`-4.0`). Wood left under a band at the stated floor, against `relief_sheet_s1.stl`:

| Band outside the lake edge | Least wood, model 15 at Z 13.5 | Least wood, model 16 at Z 11.4 |
|---|---|---|
| 6 mm (side `outside`) | 1.78 mm (lake at X325 Y220), 3.31 mm (one more lake) | 4.13 mm |
| 3 mm (side `on`) | 2.60 mm (lake at X325 Y220), 4.25 mm for the others | 4.69 mm |
| 1.6 mm | 4.23 mm | 5.21 mm |

Profile has no radial offset, so 1.6 mm is not possible without a new drawing. The proposal uses
side `on` and lifts the band of the one thin lake to Z 14.9 (10.90 + 4.0).

## Result (operator file against the proposal)

| | Operator file | Proposal |
|---|---|---|
| Shank contacts, "Lake pocket 2 mm, 11 lakes" | 463 | 111 (all at the lake at X325 Y220; 1.02 mm of wood over the flute top) |
| Shank contacts, "Lake pocket 2 mm, 1 lake" | 110 | 0 |
| V-bit moves with wood above the flute | 981 | 194 |
| River: deepest cut | 14.33 mm in one pass | no depth warning (12 passes of 1.94 mm from Z 25.5) |
| Moves above the stated feed | 0 | 0 |
| Simulated total time | 16996 s | 18891 s |
| Rapid into stock, "Drill light holes" | 1 | 1 (not fixed) |

## Open, for the operator

1. **The lake at X325 Y220.** Z 13.5 there removes the 111 contacts and leaves 2.60 mm of wood
   under the band. Z 14.9 (the proposal) keeps 4.0 mm and leaves the contacts.
2. **The river top Z.** With the top at 16.5 the river has 7 passes and the run is 1097 s shorter,
   but 159 of 60747 samples cut up to 10.65 mm deep where stock above Z 16.5 remains. The proposal
   starts at 25.5 (12 passes, many in air).
3. **The drill rapid.** The rapid into stock is at the corner hole X327.13 Y412.17 of
   `light_holes.dxf` (the highest X and Y of the 42 holes), 6.3 mm from two cavity walls. A drill
   top of 25.5 made 11 rapids enter stock, so the lead put 16.5 back. Cause not found.
4. **Not done:** the dial-in of the roughs and of the 2 mm pockets; the V-bit depth; the pin drill
   (30 mm deep, 25 mm flute); the day strip slit.

## Update 2026-11-16 (the proposal file as committed now)

The operator answered on 2026-11-15/16: the shank cannot stay in the wood; cull the river passes
that cut air; both files stay. Three code steps came from that (P35: `2755cfc6d`, `8f1b52170`,
`f31841045`, `7df21620f`), and the proposal file changed. Measured on the release GUI of
`7df21620f`, clean load, one Generate All, one simulation at 0.2 mm, cap only. This section
replaces "Changes", "Result" and open items 1 and 2 above.

### The file now

1. **River:** `depth_per_pass = 2.1`, `stock_source = "from_remaining_stock"`, top Z 16.5 as in
   the operator file. The pass ladder starts at the stock top under the curve, and the air-cut
   filter drops the parts of a pass with no wood.
2. **One band for model 15:** "Lake relief band 6 mm, 11 lakes to Z 13.5", side `on`,
   `stock_to_leave_radial = -1.0`. The tool centre is 1.0 mm inside the lake edge (the centre line
   of the 2 mm pocket tool), so the band reaches 2.0 mm outside the edge. Three lakes split at a
   narrow neck (15 rings in all); the op warns `geom.profile_loop_split`.
3. **The band for model 16** at Z 11.4 has the same offset.
4. The band at Z 14.9 is deleted.

Least wood under a band that reaches 2.0 mm outside the edge, against `relief_sheet_s1.stl`:
4.23 mm (model 15), 5.17 mm (model 16).

### Result

| | Operator file | Proposal, 2026-11-16 |
|---|---|---|
| Shank contacts, "Lake pocket 2 mm, 11 lakes" | 463 | 0 |
| Shank contacts, "Lake pocket 2 mm, 1 lake" | 110 | 0 |
| V-bit moves with wood above the flute | 981 | 315 (worst 14.36 mm over the tip) |
| River: deepest body cut | 14.33 mm | 2.05 mm (release CLI measure of P35 B) |
| River: moves, cut length | 17261, 6506 mm | 48023, 13273 mm |
| Moves above the stated feed | 0 | 0 |
| Simulated total time | 16996 s | 18080 s |
| Rapid into stock, "Drill light holes" | 1 | 1 (not fixed) |

### Still open

- The V-bit: 315 moves have wood above its 8.15 mm flute. The 6 mm band at 3 mm outside gave 194.
- The drill rapid at the corner hole (item 3 above), and item 4 above.
- Each tool size in the file is "estimated". The shank of the 2 mm tool (3.175 mm assumed) and
  the flute of the 6 mm tool decide the shank and the pin drill findings.

## Update 2026-11-16, second: the V-bit and the lake outlines

The operator, 2026-11-16: the V-bit moves with wood above the flute are "not ideal still".

**Cause (measured).** "V rivers and lakes" traces the layers `VGROOVE_V20_RIVER` and
`VGROOVE_V20_LAKE` of `vgroove_v20.dxf`. "River channel 6 mm" cuts the river layer only. On a lake
outline with no channel the V-bit goes from the cavity floor (Z 16.5) to its tip near Z 6.9:
9.6 mm deep with a cone of 8.15 mm. Instrument: `job/vbit_stock.py` in the lead scratchpad (a
height map of the back side from the exported G-code, 0.25 mm cells, the wood above each V move
in the shank footprint). It gives 399 moves where the simulation gives 315, with the same worst
place (X281.5 Y122.0 in the G-code frame, 14.36 mm over the tip).

**Change.** A new toolpath "Lake outline channel 6 mm" after the river channel: the river settings
(4 mm above `relief_sheet_s1.stl`, cutter projection, `depth_per_pass = 2.1`, remaining stock) on
the layer `VGROOVE_V20_LAKE`.

**Result** (release GUI of `7df21620f`, cap only): V-bit moves with wood above the flute 315 to
189; the worst is 9.40 mm over the tip (1.25 mm more than the cone) at X293.1 Y324.3. Shank
contacts of the 2 mm tool stay 0. Simulated total time 18228 s. "Lake relief band 6 mm, 1 lake to
Z 11.4" now cuts air only (the channel removes that wood first); it can be deleted.

**Why 189 remain (measured at three places).** The shore next to those lake outlines is steep:
within 3 mm of the outline the front surface rises from the lake level (Z 9.2 in the back frame)
to Z 12.1 to 12.3. The 6 mm channel keeps 4 mm of wood under the whole cutter, so its floor there
is at Z 16.1 to 16.3 and it removes almost no wood. The V-bit is then 8.2 to 9.4 mm deep. The
height map gives 238 moves, in the same areas.

**Options, not built:**
1. Two laps of the 2 mm tool along those outlines (Profile, side `on`, radial stock to leave
   +0.6 and -0.6, floor Z 14.9): a slot 3.2 mm wide for the 3.175 mm shank. It needs a wood
   thickness check for each lake.
2. A V-bit with a longer cone (a 20 degree bit on a 6 mm shank has a cone of about 16 mm). No
   toolpath change.
3. A shallower V-bit depth on the lake outlines (a design change of the map).

## Update 2026-11-16, third: the 2 mm slot does not close the V-bit finding

The operator chose option 1 (the 2 mm tool slot). P35 F (`51bac94ea`) added `lateral_offset` to
Project Curve. Measured on a scratch copy (`p35/wanaka/proposal_with_slot.toml` in the lead
scratchpad; release CLI, setup "1 Back"): two laps of the 2 mm tool at +0.6 and -0.6 mm from the
V-bit curves, cutter projection, 4.15 mm above the front surface, on remaining stock.

- V-bit moves with wood above the cone: 189 to 152. The worst move does not change (9.40 mm).
- The two laps add 1600 s (41 m of rapid moves each).
- **The two laps are NOT in the proposal file.**

**Why (measured, `relief_sheet_s1.stl` against the V-bit end points and the height map).** The
cone top is 8.15 mm above the tip, and the tip is 2.1 to 4.9 mm below the front surface. At 171 of
the 227 remaining points, wood within the shank radius must go down to less than 4.0 mm from the
front surface before the shank is free (least 2.73 mm, median 3.26 mm). A lap that keeps 4 mm of
wood cannot remove that wood. The 4 mm rule and the 8.15 mm cone do not fit together at a steep
shore.

**What is left to choose:**
1. A V-bit with a longer cone. No wood is thinned and no toolpath is added.
2. Laps that follow the cone top (about 5 mm above the front surface at the offset position),
   which leave 2.7 to 4 mm of wood at about 170 points.
3. Leave the 189 moves: the shank rubs over at most 1.3 mm of height (median 0.85 mm).

## Update 2026-11-16, fourth: the V-bit stays within its flute (operator ruling "Shallower V there")

The file now (code `52edf200b`, P35 G):

- "V rivers and lakes": `stock_source = "from_remaining_stock"` and
  `keep_within_flute_length = true`. Where the wood over the tip is more than the flute length
  less 0.1 mm, the op lifts the tip. No sample goes down.
- "Lake relief band 6 mm, 1 lake to Z 11.4" (id 27) is disabled: it cut only air.

Result (release GUI built from `52edf200b`, Generate All then a simulation, cap only, 0.2 mm):

| Measure | Before | Now |
|---|---|---|
| V-bit moves with wood above the flute | 189 | 0 |
| Shank or holder contacts, all toolpaths | 0 | 0 |
| Collisions, all toolpaths | 0 | 0 |
| Total time | 18228 s | 17907 s |

- The lift (agent measure on the G-code of the CLI): 252 fed moves, largest 1.395 mm at
  X291.661 Y323.325 (G-code frame), median 0.936 mm. The V-groove is shallower there by that
  much. The tip stays at least 1.023 mm below the front surface (stated depth less the lift; the
  mesh was not read again).
- The steepest fed move with a lifted end goes down 0.976 mm over 0.223 mm (77 degrees) at the
  cut feed. No generation rule lowers that feed.

Safety findings that stay (all were there before this change):

- "Drill light holes": 1 rapid into stock at X327.13 Y412.17 (cause not found).
- "Front bulk rough 18 mm": 1 move pushes the centre of the tool into wood (move 18235 at
  X116.434 Y291.638, 1.03 mm against a limit of 1.00 mm). Earlier updates of this note did not
  list it.
- "Pin drill, flip pins": 5 moves with wood above the 25 mm flute (30 mm deep).
- "Day strip slit V60": 7 moves with wood above the 5.5 mm flute (7.44 mm).
