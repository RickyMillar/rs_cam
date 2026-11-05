# The reviewer's judgement of the safe cut (saved before the engine runs)

Written 2026-11-06 before any engine run of this review. Every number comes from
`reference.py`. That script checks each printed string on its cited line and writes
`judgement.json`. Run `python3 -I reference.py`. The "Fact" column names the `FACTS` key.
Unit: 1 in = 25.4 mm (definition). The engine numbers seen before this file are in
`PREREGISTRATION.md`.

Machine of both fixtures: Shapeoko-class hobby table, 1.5 kW `VfdConstantTorque` at
24000 rpm, spindle 8000 (rivmap100) or 6000 (wanaka) to 24000 rpm, feed cap 10000 or
15000 mm/min. Stock: white oak (hardwood).

## 1. General rules from the printed sources

| # | Rule | Evidence (Fact keys) |
|---|---|---|
| G1 | A hardwood chip is not larger than the softwood chip of the same tool and size. | Onsrud 52-200 1/4 in: hardwood .005-.007 in, softwood .007-.009 in (`onsrud_hw_52200`, `onsrud_sw_52200`). Freud 1/4 in: .008-.011 against .010-.012 (`freud_quarter`). Harvey .250: .0106 against .0118 (`harvey_hard_slot`, `harvey_soft_slot`). Amana ball 1/4 in: .005-.007 against .007-.009 (`ball_hw`, `ball_sw`). |
| G2 | The Amana Spektra chart prints **one** Wood/Plywood chip. A hardwood and a softwood cell that read Spektra have the same chip. | `spektra_header` (line 5), `spektra_2f_6mm` (.0050 in = 0.127 mm). |
| G3 | The hobby charts keep feed and rpm per tool and lower the **depth** for hardwood. | Sienci 1/4 in: same 2740 mm/min, 12000 rpm; slot 5.4 mm (softwood, MDF) against 2.5 mm (hardwood), a ratio of 2.16 (`sienci_sw_quarter_full`, `sienci_hw_quarter_full`). Carbide 3D 3F 1/4 in: Pine .4 in, Mahogany .1 in, a ratio of 4.0 (`c3d_pine`, `c3d_mahogany`). |
| G4 | Two hobby charts print a hardwood full-slot depth near 0.4 D for a 1/4 in tool: 2.5 mm and 2.54 mm. | `sienci_hw_quarter_full`, `c3d_mahogany`. |
| G5 | Sienci prints the pocket depth at a stepover of 0.45 D (2.85 mm of 6.35 mm; 1.45 mm of 3.175 mm). A printed pocket width exists. | `sienci_hw_quarter_full`, `sienci_hw_eighth_full`. |
| G6 | The vendor depth table: 1 x D full chip, 2 x D minus 25 %, 3 x D minus 50 %. Freud writes "at least". | `spektra_depth_2xd`, `spektra_depth_3xd`, `freud_depth_2xd`, `onsrud_depth`. |
| G7 | The rpm band: 12000 rpm (Sienci full speed 1/4 in) to 18000 rpm (Amana chart rpm); Harvey prints 10000-20000 rpm. | `sienci_hw_quarter_full`, `spektra_rpm`, `harvey_rpm`. |
| G8 | Flat end plunge = feed / Z. Sienci prints plunge = feed / 2 on every flat row. | `ball_ramp` (Amana rule), `spektra_2f_6mm` ramp 90 in/min against feed 180 in/min. |
| G9 | Freud tells the user to start with shallow passes and the lower chip. Freud 1/4 in hardwood (0.203-0.279 mm) is 1.6-2.2 x the Spektra point (0.127 mm). | `freud_shallow`, `freud_quarter`. |
| G10 | No printed wood source gives an adaptive (thin-width, deep) cut. Fusion prints steel, hardened steel and aluminium only; aluminium: 1.5-2 D deep, 30 % D wide. | `fusion_alu`. Adaptive cells are "no reference" for depth and width. |
| G11 | Light finish width. Harvey prints Finishing at 0.1 D radial, 3 D axial, at 0.764 x the Slot-Rough chip. Without thinning and with the 3 x D table the reading is 0.5; with Garr thinning it is 0.833. The printed point is nearer the thinned reading. Judgement: a light finish pass thins. | `harvey_hard_slot`, `harvey_hard_finish`; `judgement.json` key `harvey_finish_witness`. |
| G12 | Garr's own finish row (2.5 % D radial) prints a chip of 1 % D, below its slot chip (1.5-3 % D). The Garr chart (metal) does not thin its finish chip. G11 and G12 disagree. | `garr_finish`, `garr_cpt`. |

## 2. Judgement per toolpath (white oak, the fixtures' hobby table)

"Chip" is the peak chip per tooth. "Feed at 18000" is chip x 18000 rpm x Z.

| Fixture toolpath | Tool | Depth | Width | Chip (mm) | rpm | Feed (mm/min) | Basis |
|---|---|---|---|---|---|---|---|
| rivmap100 Face 5 | 6 mm flat 2F | <= 5.6 mm at 0.45 D | <= 2.85 mm (0.45 D) | 0.114-0.127 | 12000-18000 | 2740 (Sienci) to 4572 (Spektra at 18000) | G2, G5, `flat_6mm_hardwood_*` |
| rivmap100 3D Rough | 6 mm flat 2F | no reference (G10); <= 2 D only at chip x 0.75 (G6) | no reference | 0.114-0.127 peak | 12000-18000 | chip x lift x rpm x 2 | G6, G10 |
| rivmap100 Scallop Finish | 2 mm tip tapered ball 2F | 2.0 mm step-down | 0.8 mm | 0.0762 | 18000 | 2743 | `spetool_2mm` |
| rivmap100 Project Curve | 20 deg V 5.5 mm | slot of the V | slot | 0.0762-0.178 (Amana 18/30 deg 1F) | 18000 | 50-130 in/min = 1270-3302 per flute count of the chart (1F) | `vgroove_hw` |
| wanaka Face back / Face front | 18 mm flat 2F | <= 1.0 mm | <= 0.4 D (7.2 mm) | 0.229-0.279 nearest row (3/4 in) | 17000 (Sienci) | 2590 (Sienci, 22 mm); power binds first | `face_18mm_*` |
| wanaka Back / Front bulk rough 18 mm | 18 mm flat 2F | no reference (G10) | no reference | <= 0.229-0.279 nearest row | 12000-18000 | power binds | G10 |
| wanaka Back rest 6 mm, Front 3D rough | 6 mm flat 2F | no reference (G10) | no reference | 0.114-0.127 peak | 12000-18000 | chip x lift x rpm x 2 | G10 |
| wanaka Pin drill, Drill light holes | 6 mm flat 2F | peck: no reference | n/a | n/a | 12000-18000 | plunge 1370 (Sienci) to 2286 (Spektra ramp) | G8, `drill_6mm_plunge` |
| wanaka River channel 6 mm (Project Curve) | 6 mm flat 2F | <= 2.5 mm per pass (full slot) | slot | 0.114-0.127 | 12000-18000 | 2740-4572 | G4 |
| wanaka Lake pockets, Antenna pocket | 2 mm flat 1F | <= 2 mm (1 D at the full chip, G6); Sienci prints 2.6 mm slot at 1/16 in | <= 0.44 D (Sienci 0.70 mm of 1/16 in) | 0.0508-0.0584 | 18000 | 914-1052 (1 flute) | `flat_2mm_1f_hardwood` |
| wanaka V rivers and lakes | 20 deg V 3.175 mm 1F | V depth | slot | 0.0762-0.178 | 18000 | 1270-3302 | `vgroove_hw` |
| wanaka Day strip slit, Engrave labels | 60 deg V 6.35 mm 2F | V depth | slot | 0.0635-0.0762 | 18000 | 2286 (Amana), 1730 (Sienci at 17000) | `vgroove_hw`, `sienci_hw_vcarve` |
| wanaka Face clamp pads (Pocket) | 6 mm flat 2F | <= 5.6 mm at 0.45 D | <= 2.85 mm | 0.114-0.127 | 12000-18000 | 2740-4572 | G5 |
| wanaka Front scallop finish | 2 mm tip tapered ball 2F | 2.0 mm | 0.8 mm | 0.0762 | 18000 | 2743 | `spetool_2mm` |
| wanaka Outline profile with tabs | 6 mm flat 2F | <= 2.5 mm per pass (through-cut = slot) | slot | 0.114-0.127 | 12000-18000 | 2740-4572 | G4 |

## 3. Judgement for the matrix cells (any tool, size, material)

1. Chip: inside the vendor band of the same tool, size and material. Hardwood <= softwood
   (G1); equal on a Spektra cell (G2).
2. Depth on a hobby table: hardwood full slot about 0.4 D for a 1/4 in tool (G4); softwood
   and MDF 0.85-1.6 D (G3). At the Sienci width 0.45 D: hardwood 0.88 D (1/4 in) and
   1.76 D (1/8 in) (G5).
3. A depth past 1 x D at the full band chip needs the depth-table warning (G6).
4. rpm 12000-18000 when the machine range allows it (G7).
5. Plunge = feed / Z for a flat end (G8).
6. A light finish width thins (G11), with the conflict G12 named.
7. Thin-width adaptive cuts: no reference (G10).
8. Rigid machine: no printed spec. Every rigid result is Unsourced.

## 4. Questions this judgement puts to the engine

- Q-A (brief section 2): Does a hardwood Spektra cell ship a larger chip than the softwood
  cell? By G2 the two chips are equal. By G1 the hardwood chip may be lower, never higher.
- Q-B (R37): A 3.175 mm hardwood pick of 2.2 D deep at 0.73 D wide is deeper and wider than
  the Sienci 1/8 in hardwood pocket (1.76 D at 0.46 D). Is the chip lower to pay for it?
- Q-C (R36): The hobby budget is the largest of six points. The two printed hardwood slot
  depths (G4) both sit near 0.4 D. Does the engine's hardwood slot depth exceed them?
- Q-D: Does the engine use a Freud 1/4 in row (G9) where a Spektra or Onsrud row exists?
- Q-E (R38, R41, R42): Does a light finish width thin, and how far does the feed rise?
