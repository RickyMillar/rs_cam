# Review B: V-bits and engraving (nz-wanaka-21, blind)

Reviewer B, Phase 2 of `planning/feeds_blind_review_plan_2026-11-03.md`. Inputs: briefs 00, 09, 10
and 12, the machine file, and the cited sources. This review did not read the project files, the
Suggest output or the rs_cam feeds code.

The script `review_B_calc.py` (`python3 -I review_B_calc.py`) computes every derived number in
this file and asserts each derived claim. A number with no source and no derivation is marked
"judgement".

## Summary

| Op | Tool | rpm | Chip load fz (mm/tooth) | Feed (mm/min) | Plunge (mm/min) | Depth per pass | Top risk |
|---|---|---|---|---|---|---|---|
| 9 V rivers and lakes | V20, 0.3 flat, 1 flute (check) | 16000-18000 | 0.035 / 0.045 / 0.057 | 630 / 810 / 1026 at 18000 | 300-600 through the web only; fast through the air | 1 pass 6.42 mm (derated), or 2 passes 4.32 + 6.42, or rough + 1.0 mm finish | tip breakage; line shift from deflection; burn marks at 243 path starts |
| 10 Day strip slit | V60, sharp, 2 flutes | 16000-18000 | 0.050 / 0.064 / 0.076 | 1800 / 2304 / 2736 at 18000 | 600-900 | equal area 3.67 + 5.196, or 3.0 + 4.24 + 5.196 | the slit slot above is not cut first; the V body rubs if the bite passes 5.5 mm |
| 13 Engrave day labels | V60, sharp, 2 flutes | 16000-18000 | 0.040-0.060 | 1400-2200 | 600-900 | 0.6 mm, 1 pass | burn and tear-out at stroke ends; time is set by Z moves, not by feed |

Three values per cell are conservative / centre / productive. Feeds are at 18000 rpm. At another
rpm, keep fz and scale the feed with rpm (feed = rpm x flutes x fz).

## Method and the diameter question (applies to all three ops)

1. **Chip load is a feed per tooth, and it is the same at every point of a V edge.** The feed per
   tooth is fz = F / (n x z). In a full V slot, the uncut chip thickness normal to the flank is
   h = fz x sin(phi) x cos(half angle). The cosine is 0.985 for the V20 and 0.866 for the V60
   (script). Thus the chip thickness does not change with the effective diameter D_eff. D_eff
   changes two other things: the surface speed and the strength of the tool section.
2. **The vendor chip load refers to the bit, not to D_eff.** Every V-bit chart found gives one
   chip load per tooth at a fixed 18000 rpm, for a depth of 1 x the tool diameter. Amana applies
   one band to all tip widths from 0.005 in to 0.090 in. Spetool applies 0.003 in/tooth to tips of
   0.005 in and 0.015 in. Thus the vendors key the depth derate to the nominal (body) diameter and
   do not scale fz with D_eff.
3. **Surface speed at D_eff is not a vendor input for V-bits.** The charts print the formula
   rpm = SFM x 3.82 / D but no SFM value; they fix the rpm at 18000. At 18000 rpm the V20 tip flat
   moves at 17 m/min and the top of the 6.42 mm cone at 145 m/min (script). This review sets rpm
   from the vendor operating speed and the machine range, and sets the feed from fz. In wood,
   burning comes from a thin chip and from dwell, not from a low surface speed (judgement).
4. **The rubbing floor applies along the whole edge.** Shaw gives a minimum chip of about
   0.025 mm/tooth (`sources.toml` `shaw_metal_cutting`). Because h is almost constant along the
   flank, the tip does not rub more than the flank in a straight cut. The tip rubs in a plunge
   and where the realised feed falls (corners, Z-following).
5. **Tip strength (derived).** Assume a uniform edge load q per unit length and a cone section
   with a section modulus proportional to D^3. The bending stress at height z above the tip is
   then proportional to z^2 / D(z)^3. For a flat tip a and taper b = 2 tan(half angle), the
   maximum is at z = 2a / b, where D = 3a. For the V20 (a = 0.3 mm), the critical section is
   1.70 mm above the tip, at D = 0.9 mm (script). The stress at that section does not grow with
   depth once the depth exceeds 1.70 mm. It grows with fz. Deeper cuts add total force and tool
   deflection, not tip stress. A sharp V (the V60) has its critical section at the point itself;
   the point always blunts, so measure the real tip.
6. **Power.** The script computes the material removal rate and the specific energy u_break at
   which the cut uses all the power at 18000 rpm (1125 W). No source for the specific cutting
   energy of oak in routing was found. u_break is 7.2 J/mm^3 (V20) and 2.1 J/mm^3 (V60 slit,
   heavy pass). Wood cuts with much less energy than these values (judgement). Power does not bind
   on any group B op.

### Sources

| Key | Source | What it gives |
|---|---|---|
| S1 | Amana, 1-flute 18/30/45 deg and 2-flute 60/90 deg V-groove chart, <https://www.amanatool.com/pub/media/productattachments/AMS-159-18-30-45-60-90-Degree-V-Groove-Speed-Chart-v2.pdf> | 18000 rpm, 1 x D. Hardwood, 1 flute 18-45 deg: 50-130 IPM, 0.003-0.007 in/tooth. 60 deg carbide-tipped 2 flute: 90 IPM, 0.003 in. Depth: 2 x D reduce chip load 25 %, 3 x D 50 %. |
| S2 | Amana, 2-flute solid carbide V-groove 15/60/90 deg, <https://www.amanatool.com/pub/media/productattachments/15-60-90_Degree-V-Groove-Engraving-Speed-Chart.pdf> | Hardwood 50-125 IPM, 0.003-0.007 in/tooth, 18000 rpm, 1 x D; "Ramp Down: Feed Rate IPM / # of flutes". The IPM column gives only 0.0014-0.0035 in/tooth for 2 flutes at 18000 rpm (script): the chart is not self-consistent. |
| S3 | Amana, 30 and 60 deg engraving bits (tip 0.005-0.090 in), <https://eoasaw.com/images/30%20and%2060%20Degree%20Engraving%20Speed%20and%20Feed%20Chart.pdf> | 60 deg hardwood 0.004-0.006 in/tooth, 18000 rpm, one band for all tip widths. |
| S4 | Spetool V-groove and sign-making chart, <https://community.carbide3d.com/uploads/short-url/j2UNC5QtCwKWmb5gavWDTjg0Ime.pdf> | Engraving bits, tips 0.005 and 0.015 in: hardwood 18000 rpm, 0.003 in, 54 IPM (= 1 flute). Depth: 2 x D reduce feed 30 %, 3 x D 50 %. "Due to the extremely small diameters involved, bits are not guaranteed against breakage." The scan is partly garbled. |
| S5 | `crates/rs_cam_core/tests/literature_matrix/sources.toml` `shaw_metal_cutting` | Rubbing floor about 0.025 mm/tooth. |
| S6 | `sources.toml` `shapeoko_wiki` (Shapeoko CNC A to Z) | V-bit work 16k-20k rpm; plunge 30-40 % of the feed for woods; 1/16 in hard wood 0.013-0.025 mm (lumped with metals; weak). |
| S7 | `sources.toml` `vectric_default` | Plunge 0.30-0.50 x feed. |
| S8 | `sources.toml` `onsrud_doc_rule`, `freud_router_cnc` | The same 2 x D / 3 x D chip-load reduction, from two more vendors. |
| S9 | `/home/ricky/personal_repos/project_rivmap_mono/manufacture/block/feeds_oak.toml` | v20: chip 0.0330, 20051 rpm, feed 662 (safe 530), plunge 736. v60: chip 0.0504, 10026 rpm, feed 1010 (safe 808), plunge 736. Community-data generator, 0.8 safety factor. |
| S10 | Brief 00 / machine file | rpm 6000-24000, P(n) = 1.5 kW x n / 24000, Z 2500 mm/min, XY accel 1200, Z accel 550 mm/s^2, cap 6000 mm/min. |

No Onsrud, Whiteside or Vortex V-bit chip load for hardwood was found. A search result quoted an
Onsrud plywood value for the 37-80 series (0.004-0.006) second-hand; this review does not use it.

## Op 9: V rivers and lakes (brief 09), V20 0.3 mm flat tip

Geometry (script): top width 2.56 mm at the nominal 6.42 mm; section area 9.19 mm^2; depth is
2.02 x the 3.175 mm body, so the 2 x D derate applies (S1, S4, S8).

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| Flute count | 1 (CHECK) | - | brief says 1 (map.toml); the library's 20 deg V is 2 flute | brief 09 | low: a wrong count halves or doubles fz |
| rpm | 16000-18000 | rpm | vendor operating speed 18000; V-bit work 16k-20k | S1, S2, S6 | medium |
| Chip load fz (whole edge; not tied to D_eff) | 0.035 / 0.045 / 0.057 | mm/tooth | chart low end 0.0762 x 0.75 (2 x D) = 0.057; lower values for the 0.3 mm tip and the 25 mm stick-out (judgement) | S1, S4, S8, script | medium |
| Chip at the flank (h = fz cos 10) | 0.034 / 0.044 / 0.056 | mm | derived | script | high |
| Floor | 0.025 | mm/tooth | rubbing floor | S5 | medium |
| Critical tool section | 1.70 mm above the tip, D = 0.9 mm | mm | derived (method 5) | script | medium (model) |
| Feed at 18000 rpm | 630 / 810 / 1026 | mm/min | 18000 x 1 x fz | script | medium |
| Feed at 16000 rpm | 560 / 720 / 912 | mm/min | 16000 x 1 x fz | derived | medium |
| Z-follow limit | Z hits 2500 mm/min only at a slope over 2.4 (67 deg) at 1026 mm/min | - | Z_max / F | script | high |
| Plunge, through the air in the 6 mm channel | at the cutting feed or faster | mm/min | no cut | judgement | medium |
| Plunge, into the 4 mm web | 300-600 | mm/min | 30-40 % of feed; 0.30-0.50 x feed | S6, S7 | medium |
| Depth per pass | options A, B, C below | mm | - | script | - |
| Power | MRR 153 mm^3/s at 1026 mm/min; u_break 7.2 J/mm^3 | - | 1125 W at 18000 | script | high (does not bind) |

### Depth options (Q3)

| Option | Passes (depth below the web top) | Area per pass (mm^2) | fz | Note |
|---|---|---|---|---|
| A | 6.42 | 9.19 | conservative 0.035-0.045 | 2 x D derate; full lateral force in one pass |
| B | 4.32 + 6.42 (equal area) | 4.6 + 4.6 | centre 0.045 | halves the lateral force per pass; tip stress is the same (method 5) |
| C | 5.42 + 6.42 (1.0 mm finish) | 6.80 + 2.39 | 0.045 rough, 0.057 finish | the finish pass recuts the walls of the visible lowest 2.42 mm at low force; the 0.176 mm flank shave thins the chip to about 0.51 x fz (script), so use the high fz on the finish pass |

Recommendation: option C where the line position at the front matters most, option A at the
conservative fz if time matters. One pass of 6.42 mm is not unsafe at the derated chip. The
tip stress does not depend on the number of passes. The second pass reduces deflection, chip
packing in a 2.56 mm wide, 6.4 mm deep groove, and the line shift.

### Answers to the brief 09 questions

1. **Chip load.** 0.035-0.057 mm/tooth, 1 flute, at 16000-18000 rpm: 560-1026 mm/min. The low
   end sits 40 % above the rubbing floor. The high end is the vendor low end after the 2 x D derate.
2. **Diameter.** The chip load is per tooth for the whole edge; it is the same at the tip, the
   mean and the top (method 1). The vendor values refer to the bit and are derated by depth over
   the nominal diameter (3.175 mm). The strength check refers to the critical section, D = 0.9 mm,
   1.70 mm above the tip.
3. **One pass.** Safe at the derated chip; see the options.
4. **Plunge.** A 20 deg V cannot plunge above the rubbing floor on this machine: the flank chip
   is fz_axial x sin 10 deg, and the floor needs 0.144 mm/rev, 2591 mm/min at 18000 rpm, which
   is over the Z limit (script). Thus keep the time in the plunge short:
   - Move fast through the air in the 6 mm channel to about 0.5 mm above the web (judgement).
   - Plunge the 4 mm web at 300-600 mm/min (S6, S7). The axial direction is the stiff direction of
     the tool.
   - As an option, ramp along the line over the web. A ramp puts most of the advance into the
     lateral direction, where the chip is fz cos 10.

### Risks (op 9)

- **Tip breakage.** The 0.3 mm flat and the 0.9 mm critical section are weak; stress scales with
  fz. A dull or chipped tip changes the visible line width at the front.
- **Line position.** Deflection of a 3.175 mm body at a 25 mm stick-out (ESTIMATE) moves the
  line. The force is not computed: no oak specific cutting force was found.
- **Burn marks at the path starts.** 243 path starts, each with a plunge below the rubbing floor.
  A burn on the groove wall shows through clear epoxy.
- **Realised feed on Z-following paths.** Vertices every 0.25 mm and a Z acceleration of
  550 mm/s^2 can make the planner slow down. The chip falls with the feed. At the conservative fz,
  a 50 % feed loss puts the chip under the floor (script); the productive fz keeps a margin.
- **Lake outlines.** About half the V meets air. The single flute cuts on one side only; the
  side force pushes the tool toward the air side, and the line can shift (judgement).
- **Flute count unknown.** Count the flutes before the run.

## Op 10: Day strip slit (brief 10), V60 sharp tip, 2 flutes

Geometry (script): cone 5.50 mm; full depth 5.196 mm, area 15.59 mm^2. In the planned split,
pass 1 removes 3.90 mm^2 and pass 2 removes 11.69 mm^2 (three times pass 1). The sim peak bite of
5.27 mm is inside the cone and under 1 x D (6.35 mm), so no depth derate applies.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 16000-18000 | rpm | vendor operating speed | S1-S3, S6 | medium |
| Chip load fz (bit, nominal 6.35 mm) | 0.050 / 0.064 / 0.076 | mm/tooth | vendor 0.076-0.178 (S2), 0.102-0.152 (S3), 0.076 (S1 tipped); hobby machine: low end (judgement) | S1-S3, S9 | medium |
| Chip at the flank (fz cos 30) | 0.043 / 0.055 / 0.066 | mm | derived | script | high |
| Feed at 18000 rpm | 1800 / 2304 / 2736 | mm/min | 18000 x 2 x fz | script | medium |
| Feed at 16000 rpm | 1600 / 2048 / 2432 | mm/min | 16000 x 2 x fz | derived | medium |
| Plunge (2.6-3.7 mm at the line start) | 600-900 | mm/min | ramp down = feed / flutes (900 at 1800); 30-40 % of feed | S2, S6, S7 | medium |
| Depth per pass | options below | mm | - | script | - |
| Power | heavy pass MRR 533 mm^3/s at 2736 mm/min; u_break 2.1 J/mm^3 | - | 1125 W at 18000 | script | high (does not bind) |
| Feed cap | 6000 | mm/min | does not bind | S10 | high |

### Depth options (Q3)

| Option | Passes (depth below Z 4.83) | Note |
|---|---|---|
| A (planned) | 2.598 + 5.196 | pass 2 removes 3 x pass 1 and makes the exit |
| B | 3.67 + 5.196 (equal area) | even load; same exit as A |
| C | 3.0 + 4.24 + 5.196 (equal area) | lower load per pass |
| D | 3.4 + 4.8 + 5.196 | light exit: the last pass removes 2.29 mm^2 and is 0.4 mm deep, of which 0.366 mm is in the spoilboard |

Recommendation: option D for a clean exit at the visible front, or option B to save a pass. The
last pass sets the slit width at the front face: 0.42 mm for a sharp tip 0.366 mm past Z0
(script). A worn tip makes the line narrower than planned.

### Answers to the brief 10 questions

1. **Chip load.** 0.050-0.076 mm/tooth at 16000-18000 rpm: 1600-2736 mm/min. The sharp tip has
   no D_eff to set a chip load; the chip is fz cos 30 on the whole edge.
2. **Bite of 5.27 mm.** The bite is still inside the 5.50 mm cone and under 1 x D, so the vendor
   chip load holds; use the conservative fz (1800 mm/min at 18000) on every pass. If any bite
   passes 5.50 mm, the cylindrical body rubs in the wood and no feed is safe. Then cut the 7 mm
   slit slot (op 2) before this op. Confirm the order in the simulation before the run.
3. **Two or three passes.** Three (option D) for the cleanest exit; two at equal area (option B)
   is acceptable. The planned equal-depth split puts 75 % of the material in the exit pass.

### Risks (op 10)

- **Op order.** If op 2 does not clear the slit slot, the V meets more wood than planned. Over
  the cone length the body rubs and burns.
- **Exit breakout at the front.** The front sits on the spoilboard, so the fibres have support if
  the block sits flat and tight. A gap under the block lets the fibres break at the visible edge.
- **Spoilboard.** The tip cuts 0.366 mm into the spoilboard; the spoilboard material is not stated.
- **Stick-out.** 30 mm is a set value (ESTIMATE). A longer stick-out than needed adds deflection.

## Op 13: Engrave day labels (brief 12), V60 sharp tip, 0.6 mm

Geometry (script): width 0.69 mm at 0.6 mm depth. The engaged edge is 0.69 mm long; the force is
small.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 16000-18000 | rpm | vendor operating speed | S1-S3, S6 | medium |
| Chip load fz | 0.040-0.060 | mm/tooth | under the vendor band (S2, S3) for crisp oak edges and short strokes; over the floor (judgement) | S2, S3, S5 | medium |
| Chip at the flank (fz cos 30) | 0.035-0.052 | mm | derived | script | high |
| Feed at 18000 rpm | 1440-2160 (round to 1400-2200) | mm/min | 18000 x 2 x fz | derived | medium |
| Distance to reach the feed | 0.26 mm at 1500 mm/min; under 0.5 mm up to 2000 mm/min | mm | v^2 / (2 x 1200 mm/s^2) | script | high |
| Plunge (0.6 mm) | 600-900 | mm/min | ramp down = feed / flutes; 30-40 % of feed | S2, S6, S7 | medium |
| Plunge time | 0.048 s at 750 mm/min; Z reaches 750 mm/min in 0.14 mm | s | derived | script | high |
| Depth per pass | 0.6, one pass | mm | under 0.1 x D | brief 12 | high |
| Power | negligible | - | area 0.21 mm^2 | derived | high |

### Answers to the brief 12 questions

1. **Chip load.** 0.040-0.060 mm/tooth, 2 flutes, 16000-18000 rpm: 1400-2200 mm/min. The 0.69 mm
   D_eff does not change the chip (method 1).
2. **Feed or acceleration.** At these feeds the XY axes reach the feed in under 0.5 mm, so a stroke
   of a few mm runs mostly at the commanded feed. The time goes to the 49 retracts and plunges. To
   save time, lower the retract height between strokes; a higher feed saves little.
3. **Plunge.** 600-900 mm/min, with no dwell at the bottom. The flank chip in a plunge is
   fz_axial x sin 30; at 750 mm/min and 18000 rpm it is 0.010 mm, under the floor. Thus keep the
   plunge short and do not slow it down further.

### Risks (op 13)

- **Burns at stroke ends.** Each stroke starts and ends with a plunge and a retract at a chip
  under the floor. Keep the spindle at speed and avoid any dwell.
- **Tear-out.** A shallow V in open-pored oak tears out across the grain. A sharp tool and a fz at
  the upper end (0.06) give the cleanest edge (judgement).
- **Tip wear.** A sharp tip blunts fast. At 0.6 mm depth, a blunt tip makes a visibly wider and
  shallower stroke. Use a fresh tip, or engrave before the slit wears the same tool.
- **Epoxy.** The labels are in oak (air share 0 %). If a stroke crosses the epoxy slit, use the
  same fz and do not dwell, because heat softens epoxy (brief 00).

## Items this review could not source

- A specific cutting force or energy for oak in routing. The power and deflection checks use a
  break-even bound instead.
- An Onsrud, Whiteside or Vortex hardwood chip load for V-bits.
- A vendor chip load for a 0.3 mm flat-tip 20 deg V. The nearest are Amana 18 deg 1-flute (S1) and
  Spetool engraving tips of 0.005-0.015 in (S4).
- The flute count of the V20 (brief: 1; the tool library's 20 deg entry: 2).
