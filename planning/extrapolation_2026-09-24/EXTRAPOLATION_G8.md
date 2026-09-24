# G8: Long tool and small tool loads

Status: Phase 2 (trend) done 2026-09-24; Phase 3 (fit, witness) not started;
nothing lands before the Phase 4 rulings.

Inputs: the LUT (389 rows), `fetch/G8/verified_rows.json` (14 rows),
`fetch/G8/derate_factors.json`, `fetch/G8/micro_rules.json` and the FM1
matrix `planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`.
Scripts: `scripts/g8_verified_rows.py` writes the verified rows;
`scripts/trend_g8.py` prints every table below (`python3
planning/extrapolation_2026-09-24/scripts/trend_g8.py`). Both are read-only
on the LUT. Every ratio, compliance and cap in this file is **derived**.
A printed value carries its source id.

## 0. The gap

### 0.1 The cells

`inventory_cells.csv` holds no G8 row: G8 is an overlay on shipped cells,
so `inventory.py` does not list it. `trend_g8.py` table F counts the cells
from the matrix. A cell is G8 when its `feeds_warnings` contains
`LongToolDerate`.

| Count | Value |
|---|---|
| Matrix cells | 960 |
| Shipped cells (`status` ok) | 448 |
| G8 cells | **428** (all shipped), as in `INVENTORY.md` |

`FETCH_NOTES.md` §8 quotes "470 of 498" from the R4 spec. That count has a
different basis, and this file does not reconcile it. This file uses 428.

By tool (G8 / shipped):

| Tool | 3.175 mm | 6.0 mm | 6.35 mm | 12.7 mm |
|---|---|---|---|---|
| EndMill | 60 / 60 | 60 / 60 | | |
| BallNose | 49 / 49 | 49 / 49 | | |
| TaperedBallNose | 59 / 59 | 59 / 59 | | |
| BullNose | 36 / 36 | 36 / 36 | | |
| VBit | | | 20 / 20 | 0 / 20 |

By material: softwood 130, hardwood 124, mdf 98, plywood_hardwood 76.

By operation: Adaptive 30, Adaptive3d 30, Face 32, Pocket 32, Rest 32,
Zigzag 32, Profile 24, ProjectCurve 24, DropCutter 22, HorizontalFinish 22,
RadialFinish 22, RampFinish 22, SteepShallow 20, Waterline 20, Trace 16,
Scallop 12, SpiralFinish 12, UnifiedFinish 12, Pencil 6, Chamfer 2,
Inlay 2, VCarve 2.

The CSV has no stickout column. The stickout of 45 mm is inferred from
`ToolConfig::new_default` (`crates/rs_cam_core/src/compute/tool_config.rs`).
The counts agree with it: the 12.7 mm V-bit (L/D 3.5) fires on 0 of 20
cells, and every tool of 6.35 mm or less fires on every cell. On this
inference every G8 cell has L/D 7.1 to 14.2 and takes the share 0.75. The
matrix never tests the 0.88 step.

### 0.2 The engine rules that serve these cells

| Rule | Function and file | What it does |
|---|---|---|
| Long-tool share | `feeds::long_tool_load_share` (`crates/rs_cam_core/src/feeds/mod.rs`, constants `LD_SEVERE_*`, `LD_MODERATE_*`) | 1.0 up to 4 x D stickout, 0.88 above 4 x D, 0.75 above 6 x D. The code says "repo rule, unsourced". |
| Consumer | Suggest pass 6b, `apply_aggressiveness` (`feeds/suggest/aggressiveness.rs`) | `target_share = aggressiveness x ld`. The pass lowers the depth per pass and the stepover. The feed does not take the share (ruling R4 Q7). The pass skips the finish role and drills. |
| Record | `FeedsWarning::LongToolDerate` (`feeds/mod.rs`, `calculate` Step 5b) | Stickout, diameter, ratio and share on the card. |
| Deflection model | `ToolDefinition::tip_deflection_mm` (`crates/rs_cam_core/src/tool/mod.rs`) and `force::chipload_cap_for_deflection_with_reason` (`feeds/force.rs`) | Stepped cantilever, flute section at 0.80 D, E 600 GPa carbide. The cap inverts `F = ap (Ks h + F_edge)` onto the feed. Consumers: `efficiency::deflection_chipload_ceiling_mm` (200 um bound) and `cutter_constraints` through the axial envelope (200 um rough, 50 um finish). |
| Micro size rule | `support::micro_extrapolation_refusal` (`feeds/support.rs`) | A tool under 1.5 mm refuses when its row is outside 0.5x to 2.0x its diameter. |
| Rubbing floor | `feeds::rubbing_floor`, `RUBBING_FLOOR_MM_TOOTH` 0.025 (`feeds/mod.rs`) | Warns below the band minimum or 0.025 mm, whichever is lower. The constant is unsourced. |

232 of the 428 G8 cells carry a `feeds.aggressiveness_*` diagnostic
(`cap_role` Roughing 212, SemiFinish 20). The other 196 all have `cap_role`
Finish, and pass 6b skips the share on the finish role. On those 196 cells
the share changes no number; only the warning shows.

## 1. The trend

### 1.1 Printed length rules (table A)

| Source | Axis | Printed | Acts on | Material | Verified |
|---|---|---|---|---|---|
| harvey_undercutting_sf23200 | neck length / neck diameter | 3x 120 %, 5x 100 %, 8x 80 %, 12x 65 %, 15x 55 % | chip load | metals | yes |
| niagara_hp_endmills_speeds_feeds | "Long and Extra" (no L/D) | -50 % | feed | metals | yes |
| redline_gp_carbide_endmills_p216 | "extra long" (no L/D) | -25 % | SFM (speed) | metals, graphite, plastics | yes |
| redline_endmill_tech_info | "long length" (no L/D) | -20 % | feed | metals | NO |
| redline_endmill_tech_info | "Long and Extra Long" | "up to 50%" | feed | metals | NO |
| redline_endmill_tech_info | "Long Reach or with Long Overhang" | -10 % | speed and chip load | metals | NO |
| fullerton_endmill_speeds | "extra long" (no L/D) | -25 % | SFM (speed) | metals | NO |
| amana_zrn_3d_profiling_v8 | axial depth / D (not stickout) | 2 x D -25 %, 3 x D -50 % | feed | wood | yes |

Physics statements with no factor: Helical ("rigidity as a third power
(L3) ... fourth power (D4)"; ">3x dia." necked tooling; verified), Harvey
blog ("2x stickout, 8x deflection"; not verified), Onsrud (flute length "not
more than three times the diameter", the 80 % collet rule, "Use shortest
CEL"; verified).

What the table shows:

- No wood-router vendor prints a stickout or L/D factor. The one wood rule
  on this axis is a depth-of-cut rule, and the LUT already holds it.
- The metal factors do not agree. For "long" they range from -10 % to
  -50 %, on three different quantities (chip load, feed, speed). Only
  Harvey defines the axis.
- No printed value is a share of the load target. The repo lever
  (`aggressiveness x ld`) has no printed counterpart.

### 1.2 Harvey against the cantilever law (table B)

| L/d | Harvey chip (printed) | (L/5)^3 compliance (derived) | repo share at the same L/D |
|---|---|---|---|
| 3 | 1.20 | 0.22 | 1.00 |
| 5 | 1.00 | 1.00 | 0.88 |
| 8 | 0.80 | 4.10 | 0.75 |
| 12 | 0.65 | 13.82 | 0.75 |
| 15 | 0.55 | 27.00 | 0.75 |

The log-log slope of the Harvey factor is **-0.48**. A chip rule that held
the deflection constant on a pure cantilever would have the slope -3.00.
So Harvey does not hold the deflection constant through the chip. The
compliance grows 27x from 5x to 15x; the chip falls 1.8x. Harvey's long-reach
miniature chart (SF_846100, "10x Reach Multiple") prints no chip factor. It
prints a smaller axial depth per row. That the vendor handles reach through
the depth is an inference (derived).

Harvey rounds up to the next row (worked example: ratio 6.5 takes .8) and
stops at 15x. An interpolated curve through the five points is derived.

### 1.3 The engine model (tables C and C2)

Flat end mill, carbide, ap = 1 x D, woc = 0.5 D, anchor Kc 35.1. `C` is the
tip compliance for 1 N. "cap" is the chip that holds the tip at 200 um.

Self-similar tool (flute 3 x D, shank = D). The two diameters give the
same ratios; only the absolute compliance changes (3.175 mm is 1.89x the
6 mm value):

| L/D | C/C(4D) | cap/cap(4D) at 200 um | cap at 50 um, mm/tooth | repo share |
|---|---|---|---|---|
| 2 | 0.15 | 6.72 | 7.05 | 1.00 |
| 3 | 0.61 | 1.66 | 1.68 | 1.00 |
| 4 | 1.00 | 1.00 | 0.98 | 1.00 |
| 5 | 1.67 | 0.59 | 0.55 | 0.88 |
| 6 | 2.68 | 0.36 | 0.30 | 0.88 |
| 8 | 6.06 | 0.14 | 0.07 | 0.75 |
| 10 | 11.73 | 0.06 | edge over budget | 0.75 |
| 15 | 39.77 | 0.001 | edge over budget | 0.75 |

The engine's default geometry (flute 25 mm, shank 6.35 mm), stickout sweep:

| Stickout mm | D 3.175: C um/N | C/C(20) | D 6.0: C um/N | C/C(20) | (L/20)^3 |
|---|---|---|---|---|---|
| 20 | 1.917 | 1.00 | 0.132 | 1.00 | 1.00 |
| 25 | 3.844 | 2.01 | 0.273 | 2.06 | 1.95 |
| 30 | 3.919 | 2.04 | 0.344 | 2.60 | 3.38 |
| 45 | 4.347 | 2.27 | 0.755 | 5.70 | 11.39 |
| 60 | 5.190 | 2.71 | 1.575 | 11.89 | 27.00 |

The 50 um column uses ap = 1 x D. No finish operation uses that depth, and
pass 6b does not apply the share on the finish role. So the 50 um column has
no share to compare with. Table C3 gives the finish depths.

Absolute caps at the matrix geometry (table C3: stickout 45 mm, flute 25 mm,
shank 6.35 mm, inferred). The band is the matrix `chipload_bounds_max_mm`
of the 111 G8 roughing and semi-finish cells per diameter:

| D mm | band max median (range) | cap 200 um, ap 1 x D, woc 0.5 D | cap 50 um, ap 0.2 x D, woc 0.1 D | ap_max at 200 um with the band max chip |
|---|---|---|---|---|
| 3.175 | 0.134 (0.028-0.341) | 0.184 | 0.384 | 3.94 mm (1.24 x D) |
| 6.0 | 0.185 (0.052-0.376) | 0.778 | 1.482 | above the 25 mm flute |

At ap 0.5 x D and woc 0.5 D the 3.175 mm tool holds the 50 um budget only
up to a chip of 0.032 mm. At ap 1 x D the edge force alone exceeds it.

What the model shows:

- As a ratio, the model falls much faster than the share: at 6 x D it
  allows 0.36 of the 4 x D chip; the share says 0.88.
- As an absolute value, the model is **less** strict than the share on the
  matrix cells. At 45 mm stickout the 200 um cap is above the median vendor
  band on both tools (0.184 against 0.134; 0.778 against 0.185). The
  deflection-limited depth at the band chip is 1.24 x D on the 3.175 mm
  tool and more than the flute on the 6 mm tool. On the self-similar 6 mm
  tool the 200 um cap meets 0.20 mm/tooth between L/D 10.5 and 11.
- The model's answer at high L/D is "smaller DOC or stepover" (the edge
  force is a floor). That is the lever the share acts on. The printed chip
  factors act on the other lever.
- The model's L/D behaviour depends on the tool, not on L/D alone. On the
  default 3.175 mm tool (6.35 mm shank, 25 mm flute) the compliance changes
  only 1.13x from 25 mm to 45 mm stickout. The thin flute section carries
  the compliance, and more shank adds little. The share reads L/D 7.9 to
  14.2 as one class. L/D is a fair key only for a self-similar tool.
- The Helical and Harvey-blog statements (L^3, D^4) are the law the model
  implements. They are a second witness for the model's form, not for a
  factor.

### 1.4 The one wood data point: Amana extra-long (table D)

| D mm | XL printed (mm/tooth) | Standard LUT row | Standard printed | min ratio | max ratio |
|---|---|---|---|---|---|
| 6.35 | 0.1016-0.1524 | none (no 3-flute ZrN row at 6.35 mm) | - | - | - |
| 9.525 | 0.1270-0.1778 | amana-zrn-ball-softwood-parallel-9525-3f | 0.1524-0.2032 | 0.833 | 0.875 |
| 12.7 | 0.1524-0.2032 | amana-zrn-ball-softwood-parallel-12700-3f | 0.1778-0.2286 | 0.857 | 0.889 |

The chart prints no stickout. The catalog geometry comes from toolstoday
(not verified): the XL tools have a short flute (3/4 in at 3/8, 1-1/4 in at
1/2), a relieved neck and a reach L1; the standard tools have a 2-1/4 in
flute. A derived three-section cantilever at one equal stickout (76.2 mm,
ap = 1 x D; the engine cannot model a neck) gives:

| Tool | C um/N at 76.2 mm | XL / std compliance | XL / std chip (printed max) |
|---|---|---|---|
| 46491 XL 3/8 | 0.568 | 0.65 | 0.875 |
| 46494 std 3/8 | 0.875 | | |
| 46496 XL 1/2 | 0.193 | 0.73 | 0.889 |
| 46495 std 1/2 | 0.266 | | |

At equal stickout both XL tools are stiffer than the standard tools. The
neck (D1 about 0.94 D, solid) is stiffer than the 0.80 D flute section, and
the XL flute is shorter. So a lower XL chip is not a deflection discount at
matched stickout. It is a class discount for the XL series, and it is
shallow, like Harvey's. It cannot calibrate the model, and it is not a
source for 0.88 (the share acts on the load target, not on the chip). This
paragraph rests on the unverified toolstoday geometry.

### 1.5 Micro tools (table E)

Derived minimum chip for a sharp carbide wood edge: hmin/rn 0.10-0.49
(metals and crystals only) x rn 5.91-8.0 um (carbide drill in MDF, carbide
saw in spruce) = **0.6-3.9 um**. A worn edge (HSS knives grew 6.5x) gives
up to 25 um. All inputs are unverified; the band is cross-material.

LUT rows at D <= 1.6 mm (printed, grade a except Whiteside c):

| Row | D mm | fz min-max mm | min/D % | 2 % runout um | fz min / hmin 3.9 um |
|---|---|---|---|---|---|
| amana-flat-softwood-pocket-0794-2f-spektra | 0.794 | 0.0254 | 3.20 | 15.9 | 6.5 |
| amana-flat-mdf-pocket-0794-2f-spektra | 0.794 | 0.0508 | 6.40 | 15.9 | 13.0 |
| amana-ball-softwood-parallel-0794-3f-zrn | 0.794 | 0.0191-0.0508 | 2.40 | 15.9 | 4.9 |
| amana-ball-softwood-parallel-1000-2f-zrn | 1.000 | 0.0191-0.0508 | 1.91 | 20.0 | 4.9 |
| whiteside-sc64-conical-ball-nose (grade c) | 1.442 | 0.1016 | 7.05 | 28.8 | 25.9 |
| amana-ball-softwood-parallel-1500-4f-zrn | 1.500 | 0.0127-0.0165 | 0.85 | 30.0 | 3.2 |
| amana-flat-softwood-pocket-1500-2f-spektra | 1.500 | 0.0508 | 3.39 | 30.0 | 13.0 |
| amana-ball-softwood-parallel-1587-2f-zrn | 1.587 | 0.0388-0.0635 | 2.44 | 31.8 | 9.9 |
| amana-flat-softwood-pocket-1587-2f-spektra | 1.587 | 0.0508 | 3.20 | 31.8 | 13.0 |

(The MDF twins of the ball rows print the same values.)

What the micro data shows:

- Every printed chip in the LUT is at least 2.5x the upper hmin band. The
  smallest printed chip is 10 um (amana-tapered-hardwood-parallel-3175-2f).
- The diameter at which the minimum chip would bite: hold each row's
  fz/D and solve fz = 3.9 um. The median over the 85 wood rows at D <= 3.175 mm
  is **0.12 mm**. The largest is 1.25 mm, on the low-chip Amana tapered
  finish rows (0.31 % of D). For the acceptance case (1.0 mm tapered tip
  from a 3.175 mm row): the Onsrud 77-100 row scales to 24 um (no bite);
  the Amana tapered parallel row scales linearly to 3.1 um (inside the
  band) or by the 0.61 law to 4.9 um (just above). These values are in the
  tip frame; the lookup keys on the engaged diameter (INVENTORY §4).
  With a worn edge the bite diameter moves up 6.5x.
- Runout at the vendor limit (2 % of D, PreciseBits FAQ and Harvey blog,
  both unverified) is 16-32 um on these tools. It is larger than the printed
  chip minimum on 3 of the 13 rows at D <= 1.6 mm. On the derived band,
  runout, not the edge radius, is the first limit for wood micro tools.
- The repo constant 0.025 mm is 6-40x the derived hmin. It is not an
  edge-radius floor. It is above the printed chip minimum on 5 of the 13
  micro rows (the band minimum now outranks it, ruling R4 Q9).

What the data does NOT show:

- No wood value of hmin/rn and no router-bit edge radius exists in the
  fetched sources.
- No vendor chart prints a per-size chip load below 0.79 mm.
- No printed number maps a stickout to a load-target share.
- The model's absolute force is "approximate / verify on a test cut"
  (`feeds/force.rs`). No wood measurement of tip deflection against
  stickout is in hand.

## 2. Sources

Hash: the first 12 hex digits of the sha256 of the file that `hashed_file`
names in `fetch/G8/sources.json`; the full value is there.
Grade: the grade of the entries taken from the source.

| Source id | Vendor | What it prints | Grade | URL reachable | Hash | Verifier |
|---|---|---|---|---|---|---|
| amana_zrn_3d_profiling_v8 | Amana | XL 3-flute ball/flat, wood: 1/4 0.004-0.006, 3/8 0.005-0.007, 1/2 0.006-0.008 in; depth rule | a (rows), c (derived ratio) | yes | 5cdfb9c01ec2 | 12 confirmed |
| amana_spektra_3d_profiling_v6 | Amana | 46490-K XL 1/4 in, wood 0.004-0.006 in; depth rule on chip load | a | yes | 84feaace79e1 | 2 confirmed (ap_rule wording corrected) |
| harvey_undercutting_sf23200 | Harvey | Table 1 neck L/d 3/5/8/12/15x = 120/100/80/65/55 %, metals | a | yes | a25ab09a7097 | 7 confirmed |
| harvey_miniature_long_reach_sf846100 | Harvey | "10x Reach Multiple" tools; smaller axial depth per row; no chip factor | b | yes | 64cbfde52ac1 | 0 rows; notes confirmed |
| helical_machining_guidebook_2016 | Helical | L^3 / D^4 rigidity; necked tools above 3x D | b | yes | 6d6ca79db0b4 | 2 confirmed |
| onsrud_cnc_production_routing_guide | Onsrud | flute length at most 3x D; 80 % collet rule; shortest CEL | b | yes (mirror) | 79dd5f6ef135 | 3 confirmed |
| niagara_hp_endmills_speeds_feeds | Niagara | "Long and Extra" -50 % feed, metals | b | yes (forum mirror) | ccc4e502f870 | 1 confirmed |
| redline_gp_carbide_endmills_p216 | RedLine | "extra long" -25 % SFM | b | yes | c32a1ec266a0 | 1 confirmed |
| redline_endmill_tech_info | RedLine | -20 % feed; "up to 50%" feed; -10 % speed and chip load | b | not checked | c645ac894ab6 | not verified |
| fullerton_endmill_speeds | Fullerton | "extra long" -25 % SFM | b | not checked | 4b4233afe9ce | not verified |
| toolstoday_amana_4649x_specs | ToolsToday | XL and standard geometry (D, CH, D1, L1, OAL) | - | not checked | f446a364715a | not verified |
| precisebits_calibrating_feeds_speeds | PreciseBits | test start 3 % of D per flute; 0.75 x failure feed | c | not checked | 0759b8a96201 | not verified |
| precisebits_faq | PreciseBits | runout at most 2 % of D | b | not checked | 80eec42c2bab | not verified |
| harvey_blog_optimize_miniature_end_mills | Harvey | runout at most 2 % of D; 2x stickout 8x deflection | c | not checked | 5dbdae17189a | not verified |
| harvey_blog_running_parameters_miniature | Harvey | runout under .0001 in | c | not checked | dad68ae3545a | not verified |
| micromachines_2020_hmin_effective_rake | literature | hmin 0.17 rn (copper); cites 0.10-0.49 rn for metals and KDP; rn 4.4 um | b | not checked | 1e05efbde4fb | not verified |
| materials_2019_mdf_drill_edge_radius | literature | carbide drill in MDF, rn 5.91-5.95 um | b | not checked | b6e29bbc60e0 | not verified |
| materials_2021_spruce_saw_edge_radius | literature | carbide saw in spruce, rn 8 um | b | not checked | 692d53b94c4a | not verified |
| materials_2020_hss_planer_knife_edge_radius | literature | HSS knife rn 2.08 um sharp, 13.42 um worn | b | not checked | 862e80371421 | not verified |
| materials_2025_wpc_pcd_edge_radius | literature | PCD cutter in WPC, rn about 10 um | b | not checked | 1d3743693dac | not verified |

Verifier corrections (no number changes):

1. Helical: the two sentences are on PDF pages 8 and 69, not 7 and 68 as
   `sources.json` says.
2. `FETCH_NOTES.md` §4 item 8 lists RedLine p216 as a source of "For Long
   and Extra Carbide Reduce Feed by 50%". p216 does not print that text;
   Niagara does.
3. Harvey SF_846100 prints "10x Reach Multiple" as a tool property. Cite the
   reach as printed; the chart gives no chip factor for it.
4. RedLine p216 has a "Graphite Epoxy, Plastics" row, so "metals only" is
   not exact. It has no wood row.
5. Spektra v6 `ap_rule` said "feed rate"; the chart says "chip load".
   `verified_rows.json` carries the printed text and the correction record.
6. The ZrN v8 row notes leave out tool 46591, which the block prints.

Dead ends (from `FETCH_NOTES.md` §5 and §6):

| Target | Result |
|---|---|
| Fastenal / Cleveland end mill booklet | "Access Denied" (Akamai) |
| Kennametal on MSC; Kennametal master catalog on productivity.com | bot wall; HTML, not the PDF |
| onsrud.com routing guide | HTML (the precisionboard.com mirror gave the PDF) |
| Garr HP milling guides (f, m) | HTML on 2026-09-24; old copies print only a chatter rule |
| amanatool.com search and product pages | HTTP 403; bot challenge |
| Springer J Wood Sci paper | cookie redirect / client challenge |
| Harvey Machining Advisor Pro | interactive; no printed table |
| Harvey SF_958300, ITI, Tru-Edge | no length rule ("8x Reach" is a product name; ITI prints a depth rule) |
| Whiteside and Freud | not searched on this axis |

## 3. For Phase 3

### 3.1 Long tools

Candidate forms the trend supports:

1. **Replace the share by the engine's own model (preferred).** Delete the
   step share. Let the axial envelope (`cutter_constraints`, 200 um rough)
   bind the depth and the stepover at the tool's real geometry. The form
   uses `tip_deflection_mm` and the affine force. It has no free factor.
   Range: the tools the integrator models (flat, ball, bull, tapered ball),
   materials with a Kc (all four matrix materials have one in
   `material/mod.rs`), carbide and HSS.
   Consequence on the matrix: on the 232 roughing and semi-finish G8 cells
   the load target goes from 0.75 k back to k. The envelope pulls it down
   again only above 1.24 x D depth on the 3.175 mm tool, and not inside the
   flute on the 6 mm tool (table C3). So the model is less conservative than
   the share on every matrix cell. The ruling must accept that, or keep a
   smaller deflection budget for long tools.
   Coverage gap: `axial_envelope_for_operation`
   (`feeds/suggest/axial_envelope.rs`) resolves an envelope only for
   Adaptive3d, VCarve, ProjectCurve and the 3D finishes. Of the 232 cells,
   50 have one (Adaptive3d 30, Waterline 20). The other 182 (Face, Pocket,
   Rest, Zigzag 32 each, Adaptive 30, Profile 24) have none. For those cells
   candidate 1 needs the envelope extended to the 2D operations first.
   Without it, a deleted share leaves them with no Suggest-time deflection
   guard.
2. **Keep a step share, but as a derived value.** A share keyed on L/D is
   defensible only for self-similar tools (table C). It cannot follow the
   default 3.175 mm tool, where stickout moves the compliance 1.13x
   from 25 to 45 mm.
3. **A printed factor.** No candidate: the only printed L/D factor is
   Harvey (metals, neck frame, chip load, slope -0.48), and it acts on a
   different quantity.

Second witnesses that could exist:

- Physical rule: Helical and the Harvey blog print the L^3/D^4 law (form
  only, no magnitude). One is verified.
- Simulation: the deflection gate on the wanaka fixture with the stickout
  swept (25, 35, 45, 60 mm) on the 3.175 and 6 mm tools. This compares the
  model's own gate to the share's cells. It is not independent of the model.
- A physical measurement (dial indicator on a loaded tool) would be the
  only independent magnitude witness. None is in hand.

Cells that stay refused or unmodelled:

- V-bit: `tip_deflection_mm` uses a bending fraction of 1.0 (a solid cone).
  This under-states its deflection, and no source gives a figure. The 20
  V-bit 6.35 mm cells have no model witness.
- Materials with no Kc: the force returns `None`, so the model gives no
  cap. The four matrix woods all have a Kc, but the plywood Kc is marked a
  TODO in `material/mod.rs`.
- Necked tools: the engine has no neck section (table D).

Recommendation: **per-family (model-based) for flat, ball, bull and
tapered ball; refuse for V-bit.** Retire the 0.88 / 0.75 constants in
favour of the model when the Phase 4 ruling agrees. The ruling must see
that this raises the load target on the 232 roughing cells (table C3),
and that 182 of them have no envelope today. Do not load a printed metal
factor. Keep the Amana XL rows as vendor rows for their own series;
they are not a stickout law.

### 3.2 Micro tools

Candidate forms:

1. **A minimum-chip floor from the edge radius**: fz >= k x rn, with k 0.10-0.49
   and rn 5.9-8 um (derived 0.6-3.9 um). One witness, cross-material. In the
   printed range (D >= 0.79 mm) it never binds. It would bind near 0.12 mm
   (median) and up to 1.25 mm on the lowest-chip tapered finish rows.
2. **A runout floor**: fz >= 2 % of D. Two vendors print the 2 % limit (both
   unverified). It binds on 3 of 13 printed micro rows, so a hard floor
   would contradict printed vendor rows. It can only be a warning.
3. **PreciseBits 3 % of D**: a test start point, grade c. Not a claim.

Second witnesses that could exist: a measured router-bit edge radius (not
found), a wood hmin/rn value (not found), or the simulation's chip samples
on the 1 mm tip of the wanaka "3D Finish 6" case.

Cells that stay refused: every tool below 0.79 mm (no printed row), and the
sub-1.5 mm tools that the size rule refuses today. The minimum-chip rule
does not supply a chip load; it only bounds one. G1 must supply the number.

Recommendation: **refuse as a generic claim.** Keep the micro rule as a
bound for G1 to test against: a G1 size law that gives fz under 3.9 um
(sharp) must refuse. Record the runout ratio on the card as a warning
candidate, not a gate.

### 3.3 The biggest open gap

No independent magnitude witness exists for either sub-class. The long-tool
model has its form from two sources but its absolute force is "approximate";
no wood deflection measurement against stickout was found. The micro floor
rests on metal hmin/rn ratios and on edge radii of other wood tools.
