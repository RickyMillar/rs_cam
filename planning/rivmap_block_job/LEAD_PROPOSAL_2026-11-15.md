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
