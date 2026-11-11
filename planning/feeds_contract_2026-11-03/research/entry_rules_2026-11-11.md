# Entry rules: what vendors print for ramp, helix and plunge (2026-11-11)

Research report for the feeds contract. Read-only on the code. No cargo run.
Access date for every new web source: 2026-11-11.

The question: what do tool vendors and CAM vendors print as default rules for
the entry of a router bit or an end mill into material? The app has three
unsourced sets today. The aim is one rule set for every toolpath.

**Short answer.** No wood tool vendor prints a ramp angle, a helix diameter or
a helix pitch. Two makers of hobby-class routers print ramp angles for wood,
and they print them **per material**, not per tool: 5° to 27°. The tool decides
other things: which entry kinds are permitted, the geometric cap of the angle
and of the helix radius, and the feed fraction of the entry.

## 0. Method, grades, and what the repo held before

Strength labels in this report:

| Label | Meaning |
|---|---|
| primary vendor | A tool maker or a machine maker prints the value in its own document. |
| CAM default | A CAM program prints the value in its help, or holds it as a default in its source code. |
| secondary | Trade press, a blog, or a reseller. |
| forum | A user post. Weak. |
| snippet | Read in web-search result text only; the page refused the fetch. A lead, not a source. |

"Wood" in the material column means the source names wood, plywood or MDF.
A metal source is contrast only.

**The repo already holds a full entry study: G10 (2026-09-25).** It is in
`planning/extrapolation_2026-09-24/fetch/G10/` (243 statements, 43 stored
texts, two verifier reports; summary in `FETCH_NOTES.md`). This report does not
repeat that work. It cites a G10 text as `G10:<file>`; the path is
`planning/extrapolation_2026-09-24/fetch/G10/sources/<file>.txt`. The operator
rulings Q1-Q12 on G10 are in `planning/extrapolation_2026-09-24/RULINGS.md`
("Operator rulings, 2026-09-25: G10 entry parameters").

G10 found, for wood: "Maximum ramp angle: not published", "Helix diameter:
not published", "Helix pitch: not published". Today's work looked for the
sources G10 did not reach: CAM defaults in source code, makers of hobby
routers, the Leitz Lexicon, and vendor ramp charts with a rule (IMCO).

New stored texts are in `sources_entry_rules/` (one `.txt` per source, with
URL, fetch date, hash and the quoted passage). The file names below are in
that folder unless they start with `G10:`.

`sources_entry_rules/check_entry_rules.py` asserts each derived number in
this report. Run it with `python3`; it prints "all asserts pass".

Fetches: about 55 pages or files requested; section 6 lists the ones that failed. Searches: 21.

## 1. What the sources print

### 1.1 Ramp angle, linear ramp

| # | Value printed | Tool / material | Source file | Strength |
|---|---|---|---|---|
| A1 | "We recommend using the 20° angle ramp for soft woods, plywood, and MDF like materials. The shallower 5° angle ramp is good for hardwoods and soft metals like aluminum." | Any bit. Softwood, plywood, MDF: 20°. Hardwood, aluminium: 5°. | `easel_toolpath_ramping` | CAM default (the vendor also sells hobby routers) |
| A2 | "Hardwoods (Oak, Walnut) / 6mm/0.25in, 2-flute Upcut spiral / ... 10ipm per min (~20°)" | 6 mm 2-flute up-cut. Hardwood: ~20°. | `shaper_autopass_material_recommendations` | primary vendor (router maker) |
| A3 | "Softwoods (Pine, Cedar) ... 14ipm per min (~27°)"; the same for "Composites (Plywood, MDF)" | 6 mm 2-flute up-cut. Softwood, plywood, MDF: ~27°. | same | primary vendor |
| A4 | "Plastics (Acrylic) / 3mm/0.125in, single O-flute / ... 5ipm per min (~10°)" | 3 mm single O-flute. Acrylic: ~10°. | same | primary vendor |
| A5 | "Non-ferrous metals (Brass, Aluminum) / 3mm/0.125in, single O-flute / ... 2 ipm per min (~4°)" | 3 mm single O-flute. Aluminium, brass: ~4°. | same | primary vendor |
| A6 | "The angle of the ramp is measured from the surface of your workpiece. ... 90 degrees is exactly perpendicular" | Definition of A2-A5. | `shaper_adjusting_ramp_angle` | primary vendor |
| A7 | "I see ramping is defaulted to 20 in Carbide Create Pro"; reply: "For 1/4" cutters and up, I use 3° for aluminum. Shallower for smaller cutters. 5° for wood." | Carbide Create Pro default 20° (user report). One user: wood 5°, aluminium 3°. | `carbide3d_community_ramping_defaults_aluminum`; also `G10:carbide3d_community_ramp_entry_angle` ("I notice the default is 20 degrees") | forum |
| A8 | `obj.Angle = 60` | FreeCAD Ramp Entry dressup default, any tool, any material. | `freecad_cam_ramp_entry_dressup_defaults` | CAM default (source code) |
| A9 | "In this case the ramp angle limit is 5 degrees."; "Ramping on an arc, 20 degree limit"; "a value from 1 to 45 degrees" | SketchUcam examples 5° and 20°; permitted range 1-45°. No material. | `sketchucam_ramping_howto` | CAM help (hobby) |
| A10 | 'the ramp angle is by default 2 degrees' | Fusion, any tool, any material. | `web_search_snippets_not_fetched` item 2 | snippet |
| A11 | 'Ramp Angle: Controls slope steepness (e.g., 5° to 10°).' | Carveco, example range. | same, item 3 | snippet |
| A12 | "Suggested Starting Ramp Angles: Soft/Non-Ferrous Materials: 3° – 10°", "Hard/Ferrous Materials 1° – 3°" | Metal end mills. The soft class is the nearest to plastics. | `G10:metal_harvey_ramping_success`, `G10:metal_helical_guidebook_2016` | primary vendor (metal) |
| A13 | "*Straight-Line Ramp Angle = Helical ramp angle x 5 for entry up to 1 x D." with helical 3.0° (aluminium 2024/6061/7075; composites, plastics, fiberglass), 2.5° (high-silicon aluminium, copper alloys), 2.0° (bronze) | 3-flute aluminium rougher. Straight ramp 15°, 12.5°, 10°. | `imco_m233_inch_helical_ramp_angle` | primary vendor (metal) |
| A14 | Max ramp angle per series: "Not recommended" (7 series), "2.5°" (11 series), "10°" (APT/C 5), "15°" (M202, M203), "Helical ramp x 5" (M223, M233) | Metal end mills, per tool series. | `imco_straight_line_ramp_adjustments` | primary vendor (metal) |
| A15 | SGS matrix, "ramp angle" column: centre-cutting 2-3 flute series 90; non-centre-cutting and 4-7 flute series 1 to 7; "compression" router 5; "up-cut" router 90; "down-cut" router "–" | Per tool series. The router rows carry the non-ferrous colour code only. | `G10:metal_sgs_catalog_2021` (statements `g10-metal-sgs-mx-*`) | primary vendor (metal) |
| A16 | "An angle of 90 degrees means that there is no ramping." | Definition: a plunge is the 90° case. | `cimatron_ramp_angle` | CAM help |
| A17 | none | Amana, Onsrud, Whiteside, Vortex, Freud, CMT, Leitz: no ramp angle for any wood tool. | G10 `FETCH_NOTES.md` §1 and §4; `leitz_lexicon_ed7_ch05_plunging_methods` (figures print no angle) | primary vendor: **no source found** |

Note on A2-A5. The Shaper table prints a plunge speed and an "Auto Speed" next
to each angle. The printed angle is not `atan(plunge speed / auto speed)`: the
hardwood row gives 45.0° by that formula, and the page prints ~20°. The page
does not define the relation. This report uses the printed angle only
(check script, section 7).

### 1.2 Helical entry

| # | Value printed | Tool / material | Source file | Strength |
|---|---|---|---|---|
| B1 | `obj.HelixMaxRampAngle = 5`; "The maximum allowable helix ramp entry angle (degrees)" | FreeCAD Adaptive default helix angle: 5°. Any tool, any material. | `freecad_cam_adaptive_helix_defaults` | CAM default (source code) |
| B2 | `obj.HelixMaxDiameterPercent = 100`; "Maximum (and nominal) helix entry diameter, as a percentage of the tool diameter"; C++: `helixRampTargetDiameter = min(helixRampTargetDiameter, toolDiameter)` and `helixRampMaxRadiusScaled = long(helixRampTargetDiameter * scaleFactor / 2)` | FreeCAD default helix path diameter = 100 % of D (r = 0.5 D). The code never permits more. | same | CAM default (source code) |
| B3 | `obj.HelixMinDiameterPercent = 10`; C++: `helixRampMinDiameter = max(helixRampMinDiameter, toolDiameter / 8)` | FreeCAD minimum helix path diameter: 10 % of D in the operation, D/8 in the library. | same | CAM default (source code) |
| B4 | "The maximum allowable descent in a single revolution of the helix / Set to zero to disable limitation by pitch" (`HelixMaxPitch`; no default set) | FreeCAD: a pitch cap exists; it is off by default. | same | CAM default (source code) |
| B5 | "If the value is bigger than the diameter of the tool it can leave a boss standing in the center of the helix." / "Smaller diameters can reduce the chip evacuation, create jerking machine motion and can cause tool breakage." | Fusion help: path diameter ≤ D; a small helix is bad. No default printed. | `G10:fusion_adaptive_roughing_reference` | CAM help |
| B6 | 'the Fusion built in default for 2d adaptive helix diameter is "Math.max((tool_diameter - 2 * tool_cornerRadius) * 0.95; tool_diameter * 0.25)"' | Fusion default path diameter: 95 % of the flat bottom, at least 25 % of D. | `web_search_snippets_not_fetched` item 1 | snippet |
| B7 | "use a programmed helix diameter of greater than 110-120% of the cutter diameter" | Metal. The frame (hole or path) is not defined. | `G10:metal_harvey_tool_entry`, `G10:metal_helical_guidebook_2016` | primary vendor (metal) |
| B8 | "The diameter of the starting hole will be: (tool diameter x 2) - (corner radius x 2)" | Metal. Hole = 2D − 2rc, so r = D/2 − rc. | `G10:metal_imco_helical_ramp` | primary vendor (metal) |
| B9 | "The maximum hole diameter, Dm, which can be produced in one continuous spiral, is 2 x D3"; "The pitch can never be larger then the maximum ap for the cutter concept" | Indexable cutters. Hole ≤ 2 x the flat diameter. Pitch ≤ the maximum depth of cut. | `G10:metal_sandvik_ramping` | primary vendor (metal) |
| B10 | Helical ramp angle 2.0°-3.0° at "1xD" (see A13); helical angle per series 0.5°-5° | Metal end mills. | `imco_m233_inch_helical_ramp_angle`; `G10:metal_imco_helical_ramp` | primary vendor (metal) |
| B11 | "Ramping angle = atan(helical pitch / tool-center travel distance per revolution)"; "Tool-center travel distance per revolution = (Dh - Dc) x pi" | Formula. `tan θ = pitch / (2 π r)` with `2r = Dh − Dc`. | `tungaloy_helical_milling_formulae` | primary vendor (formula) |
| B12 | 'helical interpolation ramp angles should be kept between 0.5 and 1 degree' | Seco, metal. | `web_search_snippets_not_fetched` item 4 | snippet |
| B13 | none | No wood vendor and no hobby CAM help prints a helix diameter, pitch or angle for wood. Mastercam help text found prints no default (camInstructor blog, read; not stored). | G10 `FETCH_NOTES.md` §1 | **no source found** |

No source prints a different helix rule for a tool that does not cut at its
centre. The sources say only that the angle "will vary depending on end style"
(`G10:metal_helical_guidebook_2016`) and print per-series limits (A14, A15).

### 1.3 Feed during ramp, helix and plunge

All rows but C6-C8 are G10 rows; the rulings Q1-Q5 already use them.

| # | Value printed | Tool / material | Source file | Strength |
|---|---|---|---|---|
| C1 | "To find Ramp Down: Feed Rate IPM / # of flutes" | Amana spiral plunge (up-cut and down-cut), compression, ball nose, corner radius, V-groove. Wood, plywood, MDF. Ruling Q3: this is the vertical rate. | `G10:wood_amana_spektra_plunge_v24`, `G10:wood_amana_compression_v8`, `G10:wood_amana_ball_nose_v7`; `amana_spiral_plunge_solid_wood_v5` (new: one chart for the up-cut and the down-cut numbers) | primary vendor (wood) |
| C2 | Plunge = 0.50 x feed (flat, ball, tapered ball); 0.33 x feed (V-bit) | Sienci tables, wood. | `G10:sienci_feeds_speeds_metric` | primary vendor (router maker) |
| C3 | Plunge column, 0.05-0.57 of feed by tool | IDC Woodcraft chart, wood. | `G10:wood_idc_feeds_speeds` | primary vendor (wood) |
| C4 | "many CNC users reduce ramp or plunge feed to about half the main feed rate" | Any router bit. | `G10:wood_toolstoday_understanding_feeds_speeds` | secondary |
| C5 | "When plunging into a solid, drop feed by approximately 50%." / "Plunge only in non-ferrous and short-chipping materials using slotting speeds and 25% slotting feeds." / "Reduce feed to 75% of normal" (linear ramp) | Metal: Garr 0.5; SGS 0.25; Sandvik 0.75. | `G10:metal_garr_technical`, `G10:metal_sgs_catalog_2021`, `G10:metal_sandvik_ramping` | primary vendor (metal) |
| C6 | Straight-line ramp feed: "Slotting IPT or MMPT x .65" (APT/C 5), "x .75" (2.5° series), "x .70" (M202, M203), "Same as chart" (M223, M233) | Metal, per series. | `imco_straight_line_ramp_adjustments` | primary vendor (metal) |
| C7 | "Use slotting speeds and feeds for ramp angles of 1° to 2°." / "Reduce feed to 25% when ramp angles approach 6°." | Metal: the feed falls as the angle rises. | `G10:metal_sgs_catalog_2021` | primary vendor (metal) |
| C8 | "All ramp moves are performed at the plunge rate selected for the current tool." / "If ramping is enabled, the feedrate is 1/3 of the feerate, or 100% of the plunge rate, whichever is higher." | Vectric: ramp at the plunge rate. Carbide Create Pro: max(F/3, plunge). | `G10:vectric_v12_profile_toolpath`, `G10:carbide3d_community_ramping_cc_pro` | CAM help; forum (staff post) |
| C9 | "Downcut tools CANNOT be used to plunge straight into wood and should be ramped into the part." / "Never plunge straight down with downcut tooling as this may cause fire or breakage." | Down-cut spiral, wood. | `G10:wood_vortex_catalog` | primary vendor (wood) |
| C10 | "Router bits with mainly negative cutting shear angles and HW solid router bits with RH/LD and LH/RD and router bits without plunging edge are not suitable for axial plunging." | Down-cut (negative shear) bits and bits with no plunge edge. Wood and panels. | `leitz_lexicon_ed7_ch05_plunging_methods` (new) | primary vendor (wood) |
| C11 | "for downward spirals and/or compression tools, it is best to ramp into the material." / "If you're using a compression tool on MDF or melamine, a vertical plunge may cause the cutter to overheat, as debris has no way of escaping" | Down-cut and compression. MDF, melamine, plywood. | `G10:wood_axyz_compression_bit_tip`; `woodworkingnetwork_axyz_lead_ins_ramps` (new) | primary vendor (router maker); secondary |
| C12 | "Design without plunging tip only suitable for ramp plunging. Design with plunging tip limited suitable for axial plunging." | Insert (turnblade) cutters. | `leitz_lexicon_ed7_ch05_plunging_methods` | primary vendor (wood) |

### 1.4 Plunge depth per stroke, and peck

| # | Value printed | Tool / material | Source file | Strength |
|---|---|---|---|---|
| D1 | "Depth of Cut: 1 x D Use recommended feed rate / 2 x D Reduce feed rate by 25% / 3 x D Reduce feed rate by 50%" | Amana 2-flute spiral plunge, softwood and hardwood. This is the pass depth, not a peck. | `amana_spiral_plunge_solid_wood_v5`; also `G10:wood_amana_spektra_plunge_v24` | primary vendor (wood) |
| D2 | "Axial infeed for grooving and sizing maximum 1.0 - 1.5 x D." | Leitz diamond (DP) shank cutter; chipboard, MDF, plywood. | `leitz_lexicon_ed7_ch05_plunging_methods` | primary vendor (wood) |
| D3 | "Tools with alternating twist should plunge at least 0.5 mm deeper into the material than the specified lpos. ap min = lpos. + 0.5 mm" | Compression bits: the first depth is at least the up-cut length + 0.5 mm. | same | primary vendor (wood) |
| D4 | "Max Ramp Depth": "50% of D" (2.5° and 15° series), "75% of D" (APT/C 5), "100% of D" (M223, M233) | Metal: the depth of one straight ramp. | `imco_straight_line_ramp_adjustments` | primary vendor (metal) |
| D5 | none | No vendor prints a per-peck depth for a router bit in wood. G6 found the same. | `planning/extrapolation_2026-09-24/fetch/G6/FETCH_NOTES.md` §4 | **no source found** |

### 1.5 Which entry is preferred in wood, and why

| # | Statement | Source file | Strength |
|---|---|---|---|
| E1 | "The following plunging methods are recommended for sizing and grooving tools:" Ramp plunging, Spiral plunging. Axial plunging is the third figure, with the limit of C10. | `leitz_lexicon_ed7_ch05_plunging_methods` | primary vendor (wood) |
| E2 | "The cutter can be ramped over a distance into the pocket instead of plunging vertically. This approach reduces heat build-up that damages the cutter and also reduces the load on the spindle and z axis bearings." | `vectric_v12_pocket_ramp_plunge_moves` | CAM help |
| E3 | "This behavior lowers the physical forces on a bit when it engages with the material, thus extending tool life and reducing the possibility of broken bits." | `easel_toolpath_ramping` | CAM default |
| E4 | "Ramping results in superior cut quality and prolongs the life of your router bits." | `shaper_adjusting_ramp_angle` | primary vendor |
| E5 | "In most cases, a vertical plunge is a perfectly fine way to start your cutting process." Then the compression case of C11: overheat, shorter tool life, scorch. | `woodworkingnetwork_axyz_lead_ins_ramps` | secondary (AXYZ text) |
| E6 | "Upcut spirals straight plunge/drill and have good end cutting geometry." | `G10:wood_vortex_catalog` | primary vendor (wood) |
| E7 | "Plunge cuts are the most severe way to enter the cut and the most likely to cause tearout. ... The exception to the rule is that when using compression and downcut bits, you want to get them down into the cut as fast as possible" | `cnccookbook_router_tearout_tip8` | secondary |
| E8 | "Circular ramping is always preferred to linear ramping (full slotting)" / "This is the recommended method, as it ensures the longest tool life." (helix) | `G10:metal_sandvik_ramping`, `G10:metal_harvey_ramping_success` | primary vendor (metal) |
| E9 | "Winston and I always prefer a helical plunge rather than a ramp when pocketing" (staff). "Anywhere a plunge will result in burning the wood. I use ramps on my surfacing paths." (user) | `G10:carbide3d_community_ramping_cc_pro`; `carbide3d_community_ramping_defaults_aluminum` | forum |

## 2. Agreement, spread, geometry

### 2.1 Where the sources agree

1. **A ramp or a helix is better than a plunge in wood.** Leitz, Vectric,
   Easel, Shaper and AXYZ say so (E1-E5). The reasons printed are heat at the
   tip, burn marks, load on the Z axis, tool life and tear-out.
2. **A down-cut or a compression bit must not plunge straight.** Vortex,
   Leitz and AXYZ agree (C9-C11). The reason: the chips cannot leave, so they
   pack and heat. Vortex prints "fire or breakage".
3. **An up-cut bit with a plunge point can plunge.** Vortex (E6), AXYZ (E5),
   SGS (A15: 90).
4. **A bit with no plunge edge can only ramp.** Leitz (C10, C12), Vectric
   ("cutters that cannot plunge vertically", `G10:vectric_v12_profile_toolpath`).
5. **The vertical rate is a fraction of the cut feed, about a half for two
   flutes.** Amana F/Z, Sienci 0.50, Garr 0.5, ToolsToday "about half" (C1-C5).
6. **The helix path diameter is at most the flat bottom of the tool, and CAM
   defaults sit at that limit.** FreeCAD 100 % of D (B2), Fusion 95 % (B6,
   snippet), IMCO r = D/2 − rc (B8), Sandvik (B9), Fusion help (B5).
7. **The helix is specified by angle.** FreeCAD, Fusion, IMCO and Tungaloy use
   an angle; the pitch follows (B1, B10, B11). No source prints a pitch in mm
   as a default.

### 2.2 Where the sources spread

1. **Ramp angle in hardwood: 5° (Easel) against ~20° (Shaper).** A factor of
   four between two router makers. No third wood source.
2. **Ramp angle in softwood, plywood, MDF: 20° (Easel) against ~27°
   (Shaper).** Close.
3. **CAM defaults with no material: 2° (Fusion, snippet), 5° (FreeCAD helix),
   20° (Carbide Create Pro, user report), 60° (FreeCAD ramp dressup).** The
   metal-first programs are shallow. The wood-first programs are steep.
4. **Metal vendors: 1° to 15° on tools that can ramp, by series and
   material** (A12-A15). A straight ramp is five times the helix angle at IMCO
   (A13). Harvey prints one range for both.
5. **Ramp feed: from "the plunge rate" (Vectric) to "the cut feed" (IMCO
   M233, SGS at 1-2°).** The repo's rule joins the two: section 2.3, item 4.

### 2.3 What is geometry (math), and what is a rule of thumb

Geometry. The check script asserts the numbers.

1. **Helix angle and pitch.** `tan θ = pitch / (2 π r)` (B11). So a pitch in mm
   is not one rule: the angle changes with the tool. With today's `r = 0.3 D`:

   | D (mm) | pitch 1 mm (dressup) | pitch 2 mm (3D rough) |
   |---|---|---|
   | 0.5 | 46.70° | 64.77° |
   | 1.0 | 27.95° | 46.70° |
   | 3.175 | 9.49° | 18.48° |
   | 6.0 | 5.05° | 10.03° |
   | 12.0 | 2.53° | 5.05° |
   | 18.0 | 1.69° | 3.37° |

   The operator's range is 0.5 mm to 18 mm. A fixed pitch gives the smallest
   tool the steepest entry and the largest tool the shallowest. This is the
   reverse of the one printed hint about size (A7: "Shallower for smaller
   cutters", forum).
2. **Helix radius cap.** The tool centre moves on a circle of radius r. A flat
   tool of radius R cuts the floor from r − R to r + R. No post stays at the
   centre when `r ≤ R`. For a bull nose the flat is `R − rc`, so `r ≤ R − rc`.
   This is ruling Q6, and IMCO prints the same (B8). The hole is `2r + D`. A
   ball, a tapered ball and a V-bit have no flat; a pip stays at any r > 0
   (ruling Q7 shows its height).
3. **A tool that does not cut at its centre.** `CoreZone` in
   `crates/rs_cam_core/src/tool/body.rs` derives all its limits from the core
   radius c and the blade protrusion p: straight descent `p / (2c)`, zigzag
   ramp `p / (4c)`, helix pitch from the arc, straight plunge depth p. The
   proof script is `planning/feeds_contract_2026-11-03/phases/p22_0b_core_geometry.py`.
   The vendors print per-series angles for these tools (A14, A15: 1° to 7°),
   and Sandvik prints that the angle depends on the insert radius. No vendor
   prints the formula in text. The repo's math is the source.
4. **Ramp feed against the vertical rate.** A ramp at path feed F descends at
   `F sin θ`. With the Amana rule (vertical rate F/Z), the full cut feed holds
   up to `asin(1/Z)`: 30.00° for 2 flutes, 19.47° for 3. The repo's rule
   divides by `tan θ` (conservative, `feeds/ramp.rs`): 26.57° and 18.43°. So
   Easel's 20° on a 2-flute bit descends at 0.342 F, below F/2. The wood ramp
   angles and the wood plunge fractions do not conflict.
5. **Ramp length.** `L = depth / tan θ`. For 6 mm of depth: 114.5 mm at 3°,
   68.6 mm at 5°, 34.0 mm at 10°, 28.2 mm at 12°, 16.5 mm at 20°.

Rules of thumb (no derivation in any source):

- The ramp angle by material (A1-A5, A12).
- The helix angle (B1, B10).
- "Straight ramp = 5 x helix angle" (A13).
- The share of the flat bottom that the helix uses: 95 % or 100 % (B2, B6).
- The plunge fraction (C1-C5).
- Pass depth 1 x D (D1, D2).

### 2.4 Does the rule differ per tool, as the operator expects?

Partly. The printed evidence is:

- **Per tool:** which entry kinds are permitted (C9-C12, E6); the cap of the
  angle and of the helix radius (geometry); the vertical feed fraction (C1-C3);
  the minimum first depth of a compression bit (D3).
- **Per material:** the ramp angle. Every wood source that prints an angle
  prints it by material (A1-A5). No wood source prints an angle by tool type.
- **Per tool series, in metal only:** IMCO and SGS print an angle table by
  series (A14, A15). No router-bit vendor prints such a table.

## 3. Proposal: one rule set

The rule set has one function per quantity. Every toolpath calls it with the
tool, the material and the machine. Each line names its source or says "no
source found".

Per the operator rule "never refuse a job", a "not permitted" below means a
warning on the card and in the findings, not a refusal.

### 3.1 Tool categories (four)

| Category | Tools | Why the sources separate it |
|---|---|---|
| **T1. Cuts at the centre, chips go up** | Up-cut spiral, straight flute with a plunge point, up-cut O-flute, flat and bull-nose end mills | Plunge, ramp and helix are all permitted (E5, E6, A15). |
| **T2. Chips go down** | Down-cut spiral, compression | Straight plunge is not permitted (C9, C10, C11). |
| **T3. Does not cut at the centre** | Insert bits without a plunge tip, surfacing bits, any tool with a `CoreZone` | Ramp or helix only; the angle has a geometric cap (C10, C12, section 2.3 item 3). |
| **T4. Point or ball tip** | Ball nose, tapered ball, V-bit, engraver | The axis has no cutting speed. Vendors print a plunge feed (C1-C3) and "ramping is preferred to maintain tip integrity" (`G10:metal_harvey_sf_25000`). No source prints an angle. |

The sources do not support a finer split. A single-flute O-flute has no entry
rule of its own in any source read; it appears only as the tool of A4 and A5.

### 3.2 Default ramp angle (linear ramp), all categories

One table by material class. The tool category caps it.

| Material class | Proposed default | Source | Spread |
|---|---|---|---|
| Softwood, plywood, MDF | **20°** | Easel (A1) | Shaper ~27° (A3) |
| Hardwood | **5°** (the lower printed value) | Easel (A1) | Shaper ~20° (A2). The operator decides: see 5.1. |
| Plastics | **10°** | Shaper (A4) | Harvey 3-10° (A12); IMCO 15° straight (A13) |
| Aluminium, brass | **5°** | Easel (A1) | Shaper ~4° (A5); Harvey 3-10°; IMCO 10-15° straight |

Caps and additions per category:

- T1: no cap. Source: SGS prints 90 for centre-cutting router tools (A15).
- T2: the same angle as T1. **No source found** for a different angle in
  wood. SGS prints 5 for its compression router (A15), with a non-ferrous
  colour code; it equals the hardwood value. Add: first depth ≥ up-cut length
  + 0.5 mm for a compression bit (D3).
- T3: `min(table, CoreZone::max_ramp_angle_deg())`. Source: repo math.
- T4: the same angle as T1. **No source found** for a ball, a tapered ball or
  a V-bit in any material.
- Tool diameter: **no source found** for an angle that depends on the
  diameter (one forum line, A7).

### 3.3 Helix entry, all categories

| Quantity | Proposed rule | Source |
|---|---|---|
| Radius cap | `r ≤ D/2 − rc` (flat, bull). Ball, tapered ball, V-bit: no cap; show the pip. | Geometry (2.3 item 2); IMCO (B8); rulings Q6, Q7 |
| Radius default | Keep `r = 0.3 D` now. Candidate: `r = 0.475 x (D − 2 rc)`. | 0.3 D: **no source found**, but inside every printed bound (0.125 D to 0.475 D by Fusion, snippet; 0.05 D to 0.5 D by FreeCAD). 0.475: Fusion default (B6, snippet). FreeCAD's verified default is the cap itself, 0.5 D (B2). See 5.2. |
| Radius minimum (pocket too small) | `r ≥ 0.125 D`, else fall back to a ramp | Fusion (B6, snippet). FreeCAD prints 0.05 D (B3). |
| Descent | **By angle, not by pitch in mm.** Default helix angle **5°**; `pitch = 2 π r tan θ`. | FreeCAD (B1, verified in source). Inside Harvey 3-10° (A12). IMCO prints 2-3° for aluminium and plastics (B10). **No wood source.** |
| Pitch cap | `pitch ≤ depth per pass` | Sandvik (B9); FreeCAD and Fusion hold the same field (B4) |
| T3 cap | `min(rule, CoreZone::max_helix_pitch_mm(r))` | Repo math |

At 5° and `r = 0.3 D` the pitch is 0.1649 x D: 0.08 mm on a 0.5 mm tool, 0.99 mm
on a 6 mm tool, 2.97 mm on an 18 mm tool.

Why 5° for the helix and not the material angle of 3.2: IMCO is the only
source that relates the two, and it prints the helix as the shallower one
(A13). FreeCAD's 5° is the only helix default read in a primary form. It also
equals today's dressup helix on a 6 mm tool (5.05°), so the common case does
not move.

### 3.4 Entry feed, all categories

| Quantity | Proposed rule | Source |
|---|---|---|
| Vertical (plunge) rate | The `PLUNGE_RULES` claim: flat F/Z; ball 0.50 F; tapered ball 0.50 F; 60° V-bit 0.33 F. Then the vertical-rate limit (`feeds::vertical`). | Rulings Q4, Q5 (C1, C2) |
| Vertical rate, no claim | 0.5 x cut feed | ToolsToday, Garr, Sienci (C2, C4, C5) |
| Ramp and helix feed | `min(F, vertical rate / tan θ)` | Rulings Q1, Q2; `feeds/ramp.rs`. Consistent with Amana and with the wood angles (2.3 item 4). |
| T2 straight plunge | Warn: "down-cut and compression bits must ramp" | Vortex, Leitz, AXYZ (C9-C11); ruling Q12 |
| T3 straight plunge | Depth ≤ blade protrusion p, then ramp | Repo math; Leitz (C12) |
| Plunge depth per stroke, T1 | Pass depth 1 x D at the chart feed | Amana (D1); Leitz 1.0-1.5 x D (D2) |
| Peck depth for a router bit | **No source found** (D5). Keep the repo rule and its "no source" text. | G6 |

Effect of the feed rule at the proposed angles (share of the cut feed F):

| Tool | 5° | 12° | 20° |
|---|---|---|---|
| 2 flutes, flat | 1.0 | 1.0 | 1.0 |
| 3 flutes, flat; 60° V-bit | 1.0 | 1.0 | 0.916 |
| Ball, tapered ball | 1.0 | 1.0 | 1.0 |

### 3.5 Preferred entry order

Ramp along the path or helix first; straight plunge only for T1 and T4, and
only where no ramp or helix fits. Source: Leitz (E1), Vectric (E2), Easel (E3),
Shaper (E4). Between ramp and helix, **no wood source** prints an order. The
metal sources prefer the helix (E8).

## 4. The proposal against today's three code sets

| Quantity | Today | Where | Sources say | Proposal |
|---|---|---|---|---|
| Ramp angle, 2D ops | 3° | `DRESSUP_RAMP_ANGLE_DEG`, `compute/config.rs` | Below every printed wood value (5° to 27°). Equal to the low end of Harvey's soft-metal range. A 6 mm descent needs 114.5 mm of ramp. | 5° hardwood, 20° softwood / plywood / MDF |
| Ramp angle, 3D rough | 10° | `ADAPTIVE3D_RAMP_ANGLE_DEG`, `compute/operation_configs.rs` | Inside the printed wood band. Between the two hardwood values (5°, ~20°). | The same table as 2D |
| Ramp angle, pencil and rest ramps | 12° cap | `ENTRY_RAMP_MAX_ANGLE_DEG`, `finish/pencil/emission.rs` | Inside the printed wood band. No source for a ball or tapered ball (T4). | The same table; it is a cap here, so the role stays |
| Helix pitch, 2D ops | 1 mm per turn | `DRESSUP_HELIX_PITCH_MM` | No source prints a pitch in mm. The angle runs from 46.70° (0.5 mm tool) to 1.69° (18 mm tool). | Helix angle 5°; pitch = 0.1649 x D at r = 0.3 D |
| Helix pitch, 3D rough | 2 mm per turn | `ADAPTIVE3D_HELIX_PITCH_MM` | The same. 64.77° to 3.37°. On a 6 mm tool it is 10.03°, twice the FreeCAD default. | The same one rule |
| Helix radius | 0.3 x D, capped at the flat | `HELIX_RADIUS_OVER_D` | The cap is geometry and matches IMCO. 0.3 has no source. It is inside every printed bound. CAM defaults use the cap (0.475 D to 0.5 D). | Keep the cap. Keep 0.3 until B6 is confirmed (5.2). |
| Plunge feed A | Op plunge rate | `PlungeFeedRule::OpPlungeRate` | Sourced by G10 (`PLUNGE_RULES`). | Keep; make it the one rule |
| Plunge feed B | 0.5 x first linear feed; 500 mm/min fallback | `ENTRY_PLUNGE_FIRST_FEED_FRACTION`, `ENTRY_PLUNGE_FALLBACK_MM_MIN`, `compute/catalog/entry_contract.rs` | 0.5 is printed (Sienci, Garr, ToolsToday; Amana for 2 flutes). It is wrong for 1 flute (Amana: 1.0), 3 flutes (0.333) and a 60° V-bit (0.33). No source prints 500 mm/min. | Retire. Use rule A; 0.5 x F only where no claim exists. |

Three points for the consolidation:

1. The three ramp angles do not disagree with the sources; they disagree with
   each other. The printed difference is by material, and the code has a
   difference by op family instead.
2. The two helix pitches are the weakest of today's constants. A pitch in mm
   is not a rule that any source uses, and it is steep on small tools.
3. Plunge rule B duplicates rule A with a coarser number.

## 5. Open points for the operator

1. **Hardwood ramp angle: 5° or 20°.** Two router makers, a factor of four.
   The proposal takes the lower value. Today's 10° and 12° are between the
   two. At 5° a 6 mm descent is 68.6 mm long; at 10° it is 34.0 mm. The pencil
   note in `emission.rs` says the ramps are about 90 % of that pass, so this
   choice sets the pencil's time.
2. **Helix radius default.** The Fusion expression (B6) is a search snippet;
   the forum page refused two fetch tools. To confirm it: open a 2D Adaptive
   in Fusion and read the expression of "Helical Ramp Diameter". If it reads
   as B6, the candidate `0.475 x (D − 2 rc)` has a CAM-default source.
3. **Helix angle in wood has no source.** 5° is the FreeCAD default. It is
   not a wood number.
4. **T4 tools (ball, tapered ball, V-bit) have no printed angle** in any
   material. The proposal gives them the material angle.

## 6. Dead ends and searches

Pages that refused the fetch (two tools tried for the first five):

| Page | Result |
|---|---|
| Autodesk Fusion forum, two threads (defaults 2° and the helix expression) | HTTP 403; snippet only |
| Carveco help, "Toolpaths and Machining: Ramping" | HTTP 404; snippet only |
| Easel support article on ramping | HTTP 403; the same sentence is stored from easel.com |
| Datron blog on the single-flute end mill | HTTP 403; nothing read |
| Ingersoll PrimoAluR catalogue page | HTTP 403; snippet only |
| FreeCAD wiki (`CAM_Adaptive`, `CAM_Helix`, `CAM_DressupRampEntry`) | Bot wall; the source code was read instead |
| Seco "Helical Interpolate Like A Pro" | Text loads by script; snippet only |
| mycncuk.com and talkshopbot.com threads | HTTP 503 and 403 |
| Widia AluFlash product page | Empty text |

Pages read that print no entry number: HSMWorks pocket help
(mastercam.dk copy; fields only); camInstructor Mastercam Dynamic Milling
part 5 (fields "Helix Radius", "Plunge Angle", "Entry Pitch"; no defaults);
Autodesk Fusion 3D offset roughing and bore help (fields only); Datron
cutting guide; ToolsToday "Beginner CNC mistakes"; Amana AMS-312-K booklet
(ramp-down rates of three bits only); Amana plastic O-flute chart v9 (the
F/Z rule only); two Carbide 3D threads with no number; Shaper "AutoPass and
Helix Mode" (no helix number).

Searches (21), by aim: Fusion defaults; FreeCAD Adaptive helix; Mastercam
helix defaults; Vectric ramp plunge; router-bit vendor ramp angle for down-cut
and compression; O-flute and Datron; Sandvik and other formulae; Carbide
Create default; Shapeoko e-book; Onsrud compression; Estlcam, Carveco,
Alphacam, EnRoute; Shaper AutoPass; Harvey plastics and wood; Kennametal, OSG,
YG-1 ramp angle; ball, tapered ball, V-bit entry; the G10 leads "no more than
10°" and "45 degree, 1/3 feed" (neither lead found a primary).

Not found by any search: an entry number from Onsrud, Whiteside, Freud, CMT,
SpeTool or Vortex beyond the G10 prose; a Mastercam default; an Estlcam,
Alphacam or EnRoute default; a ShopBot ramp rule.
