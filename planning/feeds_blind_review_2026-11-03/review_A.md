# Review A: roughing and slotting (blind), nz-wanaka-21

Reviewer A, Phase 2 of `planning/feeds_blind_review_plan_2026-11-03.md`. Date 2026-11-03.
Inputs: `briefs/00` and the group A briefs (01, 02, 03, 04, 05, 06, 11, 13, 14, 17), the machine
file, the tool file (geometry only), and the cited sources below. I did not read rs_cam's
suggestions, the operator's values, the feeds code or its vendor LUT.

The script `review_A_calc.py` computes every derived number in this file. Run it with
`python3 -I review_A_calc.py`. It asserts each limit claim (C1-C7 in the script).

## Summary

T% = the share of the rated spindle torque (0.597 N m) that the cut needs at the design specific
energy. "Cons" = conservative column, "prod" = productive column. Feeds in mm/min.

| Brief / op | Cons point | Prod point | Binding limit | Main risk |
|---|---|---|---|---|
| 01 face 18 mm (0, 12) | 16000 rpm, 4480, fz 0.14 | 18000 rpm, 6000, fz 0.167 | the 6000 cap (T 32 %) | entry inside stock on a brazed bit |
| 02 back bulk 18 mm (1) | 9 x 3.6 at 20000 rpm, 3200 | 9 x 2.7 at 24000 rpm, 6000; or 9 x 3.6 at 24000, 4400 | **spindle torque** | **planned 9 x 3.6 at 6000 needs 108-144 % of rated torque** |
| 03 back rest 6 mm (2) | 18000 rpm, 2250 | 20000 rpm, 4000 (arcs cut it to 2100-3600) | XY accel on arcs | **14.5 mm full-slot entry needs 145 % torque at the cons chip** |
| 04 pin drill (3) | 18000 rpm, plunge 720 | 18000 rpm, plunge 1080 | flute length, collet nut | **30 mm hole with a 25 mm flute and 30 mm stick-out** |
| 05 light holes (4) | 18000 rpm, plunge 720 | 18000 rpm, plunge 1080 | Z moves | dwell 0.5 s = 150 rev of rubbing |
| 06 river channel 6 mm (5) | one pass: 18000 rpm, 1260 | two levels of 5 mm: 2880; or one pass 1620 | torque at 10 mm full slot; chip packing | one fixed feed must suit the 10 mm worst point |
| 11 clamp pads 6 mm (11) | 18000 rpm, 3240 | 20000 rpm, 4000 | XY accel on small rings | none from feeds; force is about 6 N |
| 13 front bulk 18 mm (14) | 18000 rpm, 4500 | 18000 rpm, 6000 (cap) | the 6000 cap (T 67 %) | entry angle vs estimated core |
| 14 front rough 6 mm (15) | 18000 rpm, 2160 | 20000 rpm, 3800 | XY accel on arcs | 4.6 mm entry bite (44-70 % T, acceptable) |
| 17 outline 6 mm (18) | 1.5 mm levels, 18000 rpm, 2880 | 1.5 mm levels, 20000 rpm, 4000 | chip packing in a 25 mm slot | burning and top-edge fuzz on visible sides |

Top items:

1. **Op 1 at the cap is over the spindle.** At 9 x 3.6 mm and 6000 mm/min the cut needs about
   1.6 kW at u = 0.50 J/mm^3. The spindle gives 1.5 kW at its best (24000 rpm). The cap fits only if
   u is under 0.46 J/mm^3 and with no reserve. This agrees with the sim finding in brief 02.
2. **The 18 mm entry rule depends on two estimates.** 3 deg is under atan(1/8) = 7.13 deg, but a
   protrusion of 0.5 mm with a 10 mm core gives a limit of 2.86 deg. Measure the bit, or use 2 deg.
3. **The 6 mm deep bites (14.5 mm entry, 13 mm links in op 2) cannot be fixed with feeds.** The
   feed that keeps them inside the torque is a chip of 0.034 mm, near the rubbing floor, for the
   whole op. Fix the path.
4. **The pin holes are deeper than the flute and equal to the stick-out.** Chips cannot leave and
   the collet nut reaches the stock top.

## Method and constants

- **Chip load** fz = vf / (z x n) (Onsrud and Freud charts print this formula).
- **Radial chip thinning** (DAPRA, sources.toml `dapra_rctf`): for r = ae / D < 0.5,
  hex = fz x 2 x sqrt(r - r^2). For r >= 0.5, hex = fz. I hold hex (the real chip) inside the band,
  not fz.
- **Depth rule** (Freud and Onsrud, printed in both charts): the chart chip is for ap <= 1 x D;
  reduce it by 25 % at 2 x D and by 50 % at 3 x D.
- **Rubbing floor** 0.025 mm/tooth (Shaw, sources.toml `shaw_metal_cutting`). No point in this
  review goes under it, except the limit cases that I name as "do not run".
- **Specific cutting energy u.** Guedes et al. 2020 (6 mm cutter on a 1300 W CNC router) measured
  261-513 J/cm^3 on Goupia glabra (basic density 717 kg/m^3, near white oak) from saturated to
  anhydrous, and up to 570 J/cm^3 on a 914 kg/m^3 species. The paper prints "kJ/cm^3". That label
  cannot be correct: at its own cut (0.70 m/min x 6 x 3 mm) kJ/cm^3 gives 108 kW on a 1300 W router,
  and J/cm^3 gives 108 W (script `check_unit_label`). I use **u = 0.50 J/mm^3 as the design value**
  (high end) and show u = 0.26 J/mm^3 as the low end. Their chip was small (about 0.03 mm), and the
  energy is electrical, so the design value is probably high for chips of 0.1 mm. This is a
  conservative choice, not a measured oak value. Confidence: medium-low.
- **Power and torque.** P = u x MRR, MRR = ap x ae x vf. The VFD model in brief 00 is constant
  torque (0.597 N m) up to 24000 rpm. Thus the torque need T = u x ap x ae x fz x z / (2 pi) does
  not depend on rpm. **At a fixed chip, a higher rpm does not help power: power need and power
  available both grow with rpm.** The chip and the engagement set the load; the rpm only sets the
  feed. I keep a 20 % reserve (T <= 80 %, judgement) for adaptive engagement spikes, hard grain and
  a dull edge.
- **XY acceleration** 1200 mm/s^2: the steady feed on an arc of radius R is at most
  sqrt(1200 x R) x 60 mm/min: R 1 mm -> 2078, R 3 mm -> 3600, R 8.3 mm -> 6000.
- **Deflection** (judgement inputs: carbide E 600 GPa, steel 210 GPa, flute section 0.8 x D,
  resultant force 1.4 x tangential): values in the script; I use them only to rank risks.

### Chip load bands (advance per tooth, mm)

| Source | 6 mm (1/4 in) | 18 mm (5/8-3/4 in) | Basis |
|---|---|---|---|
| Onsrud 52-200/57-200 (hardwood, 1 x D, the chart's "GOOD" roughing tool) | 7/32 in: 0.102-0.152; 1/4 in: 0.127-0.178 | 5/8 in: 0.203-0.254; 3/4 in: 0.229-0.279 | industrial spindle |
| Freud solid carbide, hardwood | 1/4 in: 0.203-0.279 | (not listed) | 1 x D |
| Freud carbide tipped, hardwood | 1/4 in: 0.127-0.178 | 5/8 in: 0.203-0.305; 3/4 in: 0.229-0.330 | 1 x D |
| Amana Spektra spiral plunge 2F, wood, 18000 rpm | 6 mm: 0.127 | n/a | sources.toml `amana_spektra_plunge_v24` |
| Shapeoko CNC A to Z, "hard wood & metals" | 1/4 in: 0.025-0.05 | n/a | hobby machine, lumps wood and metal |
| rivmap `feeds_oak.toml` `flat_6` | 0.0487 (feed_safe gives 0.039) | n/a | generated, safety 0.80 |

My working bands, between the hobby and the vendor values (judgement, inside the sourced span):

- 6 mm 2F up-cut: hex 0.05 (cons) to 0.08-0.10 (prod) on side cuts; full slots per the depth rule and
  the torque.
- 18 mm 2F: hex 0.10 (cons) to 0.13-0.17 (prod) where the torque allows. The vendor bands are
  higher; the 6 mm shank on a belt machine is the reason to stay low.

### The 18 mm bit: what it probably is

A 2-flute 18 mm head with a 20 mm cut, a 6 mm shank and a non-cutting centre core is most likely a
brazed carbide-tipped straight bit on a steel body (judgement; vendor not stated). Freud says,
verbatim: "Carbide tipped bits should not be used to drill directly into the work piece." I use the
Freud carbide-tipped hardwood row for it. Check the rated maximum rpm printed on the bit or its
pack before you run it at 24000 rpm (Freud: "always check to ensure the rated speed of the router
bit is higher than or equal to the maximum speed").

Shank check (judgement inputs): at 80 % torque the tangential force at r = 9 mm is 53 N. The bending
stress at the collet in a 6 mm steel shank is about 72 MPa, and the deflection of the 5 mm free
shank is about 0.01 mm. **The power limits this tool first, not the shank** (brief 02 question 4).
The machine frame and the collet are not in this estimate.

### Entry rule for the 18 mm non-centre-cutting bit

- Straight plunge: never deeper than the protrusion (1 mm, ESTIMATE). Prefer no straight plunge
  into wood at all (Freud rule above).
- Linear ramp: angle <= atan(protrusion / core). With 1 mm / 8 mm: 7.13 deg. With the plausible
  error of the estimates the limit moves: 0.5 / 8 -> 3.58 deg; 1 / 10 -> 5.71 deg;
  0.5 / 10 -> 2.86 deg. **3 deg is safe only if the protrusion is >= 0.42 mm (8 mm core) or
  >= 0.52 mm (10 mm core).** Until the bit is measured, use **2 deg** (cons) or 3 deg (prod, after
  measurement).
- Helix: the script computes the descent while a floor point stays under the core. At helix radius
  5.4 mm (0.3 x D) and pitch 2.0 mm the worst descent is 0.53 mm (8 mm core) or 0.75 mm (10 mm
  core). Max pitch: 3.77 mm/rev (1 mm / 8 mm), 1.88 (0.5 / 8), 1.33 (0.5 / 10).
  **Use pitch <= 1.3 mm until measured.**
- Ramp feed: the Z part is small (6000 x sin 3 deg = 314 mm/min, under Z 2500). The leading edge is
  a full-width cut only 0.94 mm deep over one diameter at 3 deg. Keep the ramp at the cutting feed
  or at 50-70 % of it (judgement); a slow ramp burns oak.
- These rules assume that the core is concentric and flat. Measure the core diameter and the
  protrusion with a caliper and a depth gauge.

## Sources

| Key | Source |
|---|---|
| ONS | LMT Onsrud, Hard Wood Cutting Data Recommendations, PCT-19 p.115, https://www.onsrud.com/images/Hard%20Wood.pdf (fetched and read 2026-11-03) |
| FRD | Freud, Router Bit Feed Rates and Speeds for CNC, 2017-08-22, https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf (fetched and read 2026-11-03) |
| AMA | Amana Spektra Spiral Plunge 2/3 Flute v24, as quoted in `crates/rs_cam_core/tests/literature_matrix/sources.toml` key `amana_spektra_plunge_v24` |
| SAZ | Shapeoko CNC A to Z, feeds and speeds, as quoted in `sources.toml` key `shapeoko_wiki` (plunge 30-40 % of feed for wood; DOC 10-50 % for soft materials; do not go under 5 % DOC) |
| DAP | DAPRA radial chip thinning, `sources.toml` key `dapra_rctf` |
| SHW | Shaw, Metal Cutting Principles, rubbing floor 0.025 mm, `sources.toml` key `shaw_metal_cutting` |
| GUE | Oliveira Guedes et al. 2020, Cutting energy required during the mechanical processing of wood species at different drying stages, Maderas Cienc. Tecnol. 22(4):477-482, https://www.redalyc.org/journal/485/48565177006/48565177006.pdf |
| FO | `/home/ricky/personal_repos/project_rivmap_mono/manufacture/block/feeds_oak.toml` |
| M00 | `briefs/00_machine_and_material.md` and `~/.config/rs_cam/machines/shapeoko_pro_xxl_scylla.toml` |
| CALC | `review_A_calc.py` (derivation) |

I did not use the `[chip_load] k0 p q` table in the machine file. It is an rs_cam model input, not
a machine fact.

---

## 01 Face back and face front, 18 mm (ops 0 and 12)

Cut: ae 14.4 mm (0.8 x D), ap 0.5 mm. r = 0.8 > 0.5, so no chip thinning: hex = fz.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 16000 (cons) / 18000 (prod) | rpm | vendor charts quote 18000; torque is not binding | FRD, AMA, CALC | medium |
| chip load fz = hex | 0.14 (cons) / 0.167 (prod) | mm | under the vendor band 0.20-0.33, for the 6 mm shank | ONS, FRD | medium |
| feed | 4480 (cons) / 6000 (prod) | mm/min | fz x 2 x n | CALC | medium |
| plunge | none into wood; start rows in air | - | brazed bit, no centre cut | FRD | high |
| ramp angle / feed | <= 2 deg until measured (3 deg after); ramp at cutting feed | deg | atan(p / core) with estimate error | CALC | medium |
| stepover | 14.4 (0.8 x D) is fine; 12-14.4 | mm | the core trails behind the blades in a side move; no rule binds | judgement | medium |
| depth per pass | 0.5 (one pass); up to 1.0 if the board is cupped | mm | 1.0 mm keeps T at about 54-64 % | CALC | medium |
| power at chosen point | 269 W (cons) / 360 W (prod) vs 1000 / 1125 W | W | u = 0.50; T 27 % / 32 % | GUE, CALC | medium-low |

Answers:

1. Chip: 0.14-0.17 mm per tooth. No chip thinning at 0.8 x D. The vendor band (0.20-0.33) is for an
   industrial machine; I stay under it.
2. **The 6000 mm/min cap binds first.** The torque is only 32 % at the cap. The rpm range does not
   bind: 15000-20000 rpm gives fz 0.15-0.20 at 6000.
3. A row that must start inside the stock: ramp in at <= 2 deg (measure, then 3 deg) along the row,
   or a helix with pitch <= 1.3 mm. At 0.5 mm depth even a straight drop is under the 1 mm
   protrusion, but the bit is a brazed bit and must not drill (FRD). Prefer air starts.
4. Yes. 6000 mm/min at 18000 rpm is right for this face. The row-end stop distance at 6000 is only
   4.2 mm, so the acceleration does not matter on 431 mm rows.

Risks:

- The 0.5 mm skim on a cupped board leaves unfaced spots. Measure the board flatness first. If it
  is more than 0.4 mm, take 1.0 mm or two passes.
- The face of op 12 crosses the epoxy line of the slit (0.4-1.0 mm wide). Keep the chip (do not slow
  down there); heat comes from rubbing, not from a thick chip (judgement).
- The pins must be below Z 25.0 before op 12. A pin above the face is a crash.
- A large head on a 6 mm shank at 18000 rpm: check the bit's rated rpm and its balance.

## 02 Back bulk rough, 18 mm (op 1)

Planned: geodesic adaptive, ap 9.0 mm, ae 3.6 mm (0.2 x D, thinning factor 0.80), second level
5.5 mm.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 20000 (cons) / 24000 (prod) | rpm | torque is rpm-independent; high rpm gives feed | M00, CALC | medium |
| chip load (9 x 3.6) | fz 0.080 / hex 0.064 (cons); fz 0.092 / hex 0.074 (prod, the 80 % torque limit) | mm | torque limit, not chart limit | GUE, CALC | medium-low |
| feed (9 x 3.6) | 3200 (cons) / 4400 (prod) | mm/min | fz x 2 x n | CALC | medium-low |
| feed (9 x 2.7, alternative) | 4000 at 20000 rpm (cons) / 6000 at 24000 rpm (prod) | mm/min | hex 0.071 / 0.089; T 65 % / 81 % | CALC | medium-low |
| plunge | no straight plunge into wood | - | non-centre-cutting, brazed | FRD, CALC | high |
| ramp angle / feed | <= 2 deg until measured, then <= 3 deg; helix pitch <= 1.3 mm (r 5.4) until measured | deg, mm | core and protrusion are ESTIMATES | CALC | medium |
| stepover | 2.7 (0.15 x D) prod; 3.6 only at <= 4400 mm/min | mm | torque | CALC | medium-low |
| depth per pass | 9.0 (keep; flute 20 mm, 0.5 x D) | mm | under 1 x D: no depth reduction (FRD/ONS rule) | FRD, ONS | medium |
| power at chosen point | 1215 W at 24000 rpm (9 x 2.7 @ 6000) vs 1500 W; 1192 W (9 x 3.6 @ 4400) vs 1500 W | W | u = 0.50; low u 0.26 gives about 630 W | GUE, CALC | medium-low |

Depth x stepover options (u = 0.50, T <= 81 %):

| Option | ap x ae (mm) | rpm | fz / hex (mm) | feed (mm/min) | T % | MRR (cm^3/min) |
|---|---|---|---|---|---|---|
| A cons | 9 x 3.6 | 20000 | 0.080 / 0.064 | 3200 | 69 | 104 |
| A prod | 9 x 3.6 | 24000 | 0.092 / 0.074 | 4416 | 79 | 143 |
| B cons | 9 x 2.7 | 20000 | 0.100 / 0.071 | 4000 | 65 | 97 |
| **B prod** | **9 x 2.7** | **24000** | **0.125 / 0.089** | **6000** | **81** | **146** |
| C (2 levels) | 4.5 x 3.6 | 18000 | 0.167 / 0.133 | 6000 | 72 | 97 |
| D | 9 x 1.8 | 24000 | 0.125 / 0.075 | 6000 | 54 | 97 |
| PLANNED at cap | 9 x 3.6 | 24000 | 0.125 / 0.100 | 6000 | **108** | 194 |
| PLANNED at cap | 9 x 3.6 | 18000 | 0.167 / 0.133 | 6000 | **144** | 194 |

Answers:

1. **No.** At 9 x 3.6 the 6000 mm/min cap needs 1620 W (u = 0.50). That is 108 % of the rated
   torque at 24000 rpm and 144 % at 18000 rpm. The cap fits under the rated power only if u is
   0.46 J/mm^3 or less, with no reserve. At 9 x 3.6, the feed for 80 % torque is 4440 mm/min at
   24000 rpm and 3330 at 18000 rpm. The sim finding (peak power over the curve) agrees.
2. Entry: no straight plunge. Ramp at 2 deg until the bit is measured. 3 deg has a margin only if
   the protrusion is >= 0.42 mm (8 mm core). With the estimates as given (1 mm / 8 mm) 3 deg is
   42 % of the 7.13 deg limit, but a 0.5 mm / 10 mm bit makes 3 deg unsafe. The helix at pitch 2.0
   puts 0.53-0.75 mm of descent under the core; use pitch <= 1.3 mm until measured.
3. See the options table. Deep and narrow (B: 9 x 2.7 at 6000) gives the most MRR inside the
   torque. Shallow and wide (C) runs at the cap with a thicker chip but removes 33 % less per minute.
4. The power (torque) limits this tool first. The shank stress is about 72 MPa at 80 % torque.

Risks:

- Adaptive engagement is not constant at the region edges and corners. The 20 % reserve is for
  that. If the planner can exceed 0.2 x D locally, use option A cons.
- Level 2 (5.5 mm in the board pocket) at 3.6 mm: 6000 at 20000 rpm gives T 79 %. It is acceptable
  with the same feed as level 1.
- Chip evacuation from a 9 mm pocket with a 2-flute bit and no air: use an air blast (judgement).
- If the bit is rated under 24000 rpm, option B prod is not available. Then use 9 x 2.7 at
  (rated rpm) x 0.25 mm per rev.
- Low chip is the burn risk, not high chip: do not go under hex 0.05 to "be safe".

## 03 Back rest, 6 mm (op 2)

Planned: ae 1.2 (0.2 x D, factor 0.80), ap 1.2. Air share about 63 %.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 18000 (cons) / 20000 (prod) | rpm | vendor charts at 18000; feed is arc-limited | AMA, CALC | medium |
| chip load | fz 0.0625 / hex 0.05 (cons); fz 0.10 / hex 0.08 (prod) | mm | between SAZ and ONS bands | SAZ, ONS, DAP | medium |
| feed | 2250 (cons) / 4000 (prod) | mm/min | the arcs cut it: R 1 mm -> 2078, R 3 mm -> 3600 | CALC | medium |
| plunge | 720-1080 (only if centre cutting is confirmed) | mm/min | axial 0.02-0.03 mm/tooth at 18000 | AMA, CALC | medium |
| ramp angle / feed | 3 deg; helix pitch 1.0-2.0 mm; at the cutting feed | deg | judgement | judgement | low |
| stepover | 1.2 (0.2 x D); up to 1.8 (0.3 x D) | mm | power is 2-4 % of torque | CALC | medium |
| depth per pass | 1.2 (set by the V20 budget; keep) | mm | generator constraint | brief 03 | high |
| power at chosen point | 27 W (cons) / 48 W (prod) vs 1125 / 1250 W | W | u = 0.50 | GUE, CALC | medium |

Answers:

1. 18000-20000 rpm, hex 0.05-0.08 (fz 0.0625-0.10), feed 2250-4000 mm/min. On short arcs the
   machine does not reach 4000; the real chip then falls. At 2250 the chip stays above the rubbing
   floor at any arc of R >= 1 mm.
2. Power gives room for a deeper cut (e.g. 3.6 x 0.9 mm at fz 0.07, hex 0.05), but the generator
   sets 1.2 mm from the V20 budget. Keep 1.2 unless the budget changes. A wider ae (1.8 mm) cuts the
   level count only if the planner then needs fewer rings; it does not cut the levels.
3. **No feed is safe for the 14.5 mm deep entry in practice.** A full slot 14.5 mm deep (2.4 x D)
   at the cons chip needs 145 % of the rated torque. The 80 % torque chip there is 0.034 mm, close
   to the rubbing floor (0.025), and it would set the feed of the whole op to about 1240 mm/min.
   The 13 mm links are the same case. The tool deflection at that load is about 0.06 mm and the
   bending stress about 470 MPa (judgement inputs). Fix the path; do not tune the feed for it.
4. Ramp 3 deg, helix radius 0.3 x D, pitch 1-2 mm (judgement; no source found for a ramp angle in
   oak). Confirm that the bit cuts at the centre.

Risks:

- **Dangerous: the 14.5 mm entry and the 13 mm link crossings** (stall, breakage, burn).
- 63 % air plus short arcs: the real feed is under the command. Chips stay above the floor at the
  cons point; the prod point gives more margin against rubbing.
- A gouge into the hollow thins the 4.0 mm web.

## 04 Pin drill, 6 mm end mill (op 3)

Planned: plunge 30 mm (25.5 oak + 4.5 spoilboard), peck 6 mm, flute 25 mm, stick-out 30 mm.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 16000-18000 | rpm | AMA chart rpm | AMA | medium |
| chip load (axial) | 0.02 (cons) / 0.03 (prod) per tooth | mm | 30-50 % of the Amana ramp-down 0.0635 | AMA, SAZ | medium |
| feed (plunge) | 720 (cons) / 1080 (prod) | mm/min | fz x 2 x n; under Z 2500 | CALC | medium |
| peck | 6 mm to 12 mm depth, 4 mm to 18 mm, then 3 mm; full retract each peck | mm | judgement (ONS drill has no peck guidance, sources.toml) | judgement | low |
| ramp | n/a (or helix if a smaller tool interpolates) | - | - | - | - |
| stepover | n/a | - | - | - | - |
| depth | **<= 23 mm with this tool** (flute 25 - 2) | mm | chips must leave the flute above the stock top | judgement | medium |
| power at chosen point | 170-254 W vs 1125 W | W | u x pi r^2 x vz | GUE, CALC | medium |

Answers:

1. 18000 rpm, plunge 720-1080 mm/min (axial chip 0.02-0.03 mm/tooth). The Amana ramp-down column
   gives 0.0635 mm/tooth for an industrial machine; I use 30-50 % of it. This is inside the
   Shapeoko 30-40 % of feed rule (648-1440 mm/min).
2. Make the peck shorter as the hole gets deeper (6, 6, 4, 4, 3, 3 ...) with a full retract. The
   chips in a 6 mm hole in oak pack after about 2-3 x D (judgement).
3. **No.** At 30 mm depth with 30 mm stick-out the collet nut face meets the stock top (0 mm
   clearance, worse if the nut is wider than the hole edge region and the stock is not flat). And
   for the last 5 mm the plain 6 mm shank is in a 6 mm hole: no flute to carry chips up, rubbing on
   the wall, heat and a swollen fit.

Risks:

- **Dangerous: collet nut contact and chip packing below 25 mm.** Options: (a) a tool with a flute
  >= 33 mm and a stick-out >= 36 mm; (b) drill the oak to 23 mm, then finish the through-depth with
  a long drill (brad-point) in a second op; (c) cut the spoilboard pin holes once as a separate
  fixture op and make the stock holes only through the 25.5 mm with a long flute.
- A plunge with an end mill burns the wall at low axial chip. Do not go under 0.02 mm/tooth.

## 05 Light holes, 6 mm end mill (op 4)

Planned: 42 holes, 8 mm deep (1.33 x D), peck 6 mm, dwell 0.5 s.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 18000 | rpm | AMA | AMA | medium |
| chip load (axial) | 0.02 (cons) / 0.03 (prod) | mm | as op 3 | AMA, SAZ | medium |
| feed (plunge) | 720 (cons) / 1080 (prod) | mm/min | fz x 2 x n | CALC | medium |
| peck | one peck of 4 mm + 4 mm, or one full plunge of 8 mm at the cons feed | mm | 1.33 x D is shallow | judgement | medium |
| dwell | 0 s (at most 0.1 s) | s | 0.5 s = 150 rev at 18000 with no feed | CALC | medium |
| stepover / ramp | n/a | - | - | - | - |
| power at chosen point | 170-254 W vs 1125 W | W | as op 3 | CALC | medium |

Answers:

1. 18000 rpm, 720-1080 mm/min plunge.
2. The dwell does not help. A centre-cutting flat end mill leaves a flat bottom without it. 150 rev
   of rubbing on one spot burns oak. Set the dwell to 0.
3. 8 mm is 1.33 x D: one full plunge at 720 mm/min is acceptable. A 4 + 4 peck with a full retract
   is the safer choice. 6 + 2 is fine too; the peck order does not matter much at this depth.

Risks:

- Holes in the hollow band start in air: no risk from feeds.
- The light holes need the bit to cut at the centre. "Assumed yes" in the brief: confirm it.

## 06 River channel, 6 mm full slot (op 5)

Planned: one pass, 100 % radial, ap up to 10.02 mm (1.67 x D), most of the length 0-2.5 mm, near
the lakes 4.5-5.5 mm. No entry dressup.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 18000 | rpm | AMA | AMA | medium |
| chip load (one pass, 10 mm) | 0.035 (cons) / 0.045 (prod); limit at 80 % torque 0.050 | mm | torque; depth rule -17 % at 1.67 x D | GUE, FRD, CALC | medium-low |
| feed (one pass) | 1260 (cons) / 1620 (prod) | mm/min | fz x 2 x n | CALC | medium-low |
| chip load (levels of <= 5 mm) | 0.06 (cons) / 0.08 (prod) | mm | T 48 / 64 % | CALC | medium |
| feed (levels of <= 5 mm) | 2160 (cons) / 2880 (prod) | mm/min | fz x 2 x n | CALC | medium |
| plunge at path starts | <= 720 mm/min straight down, or a ramp of 3-5 deg along the path | mm/min | as op 3 | AMA, judgement | medium |
| stepover | 6 (full slot, fixed by the op) | mm | - | brief | high |
| depth per pass | <= 5 mm (0.85 x D) prod; one pass at 10 mm only at <= 1620 mm/min | mm | torque, chip packing | CALC, FRD | medium |
| power at chosen point | 812 W (one pass prod) vs 1125 W; 720 W (5 mm prod) vs 1125 W | W | u = 0.50 | GUE, CALC | medium-low |

Answers:

1. One pass at 10 mm: 18000 rpm, fz 0.035-0.045, 1260-1620 mm/min. The 80 % torque limit there is
   fz 0.050 (1800 mm/min). The chip 0.035-0.045 is in the Shapeoko hobby band and 17 % or more under
   the vendor band, as the depth rule asks. **One pass is safe at the deepest point only at this low
   feed.** The deflection at the limit is about 0.07 mm (judgement inputs): the floor stays, the wall
   moves a little. Chip packing in a 10 mm slot with an up-cut bit and no air is the larger risk.
   Use an air blast.
2. **Better: depth levels.** Two levels of <= 5 mm at 2880 mm/min (fz 0.08) are faster where the
   channel is deep and give the same result where it is shallow (the shallow parts need one level
   only). If the op must stay one pass, a feed that follows the local depth is right:
   fz(ap) = min(0.08, 0.050 x 10.02 / ap). It gives fz 0.08 for ap <= 6.3 mm and 1800 mm/min at
   10 mm. If neither is possible, run the whole path at 1620 mm/min.
3. A straight plunge to the local depth (up to 10 mm) with a centre-cutting 6 mm bit: at most
   720 mm/min (axial 0.02 mm/tooth). Better: start each path in air above the floor and ramp down
   along the path at 3-5 deg (judgement). Confirm the centre cutting.

Risks:

- The 0.25 mm vertices with Z changes: grblHAL plans the Z part with 550 mm/s^2; on steep parts the
  real feed falls and the chip falls. At 1620 mm/min the Z rate is under 2500 for any slope under
  1.5 (CALC: 2500 / 1620).
- A full-width slot burns where the feed slows. Do not set the feed lower than the cons column.
- A deflected wall changes the V width at the front: keep the deep parts at the low feed.

## 11 Face clamp pads, 6 mm (op 11)

Planned: pocket, ae 2.4 (0.4 x D, factor 0.98), ap 0.5 mm, about 60 % air, side pushers only.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 18000 (cons) / 20000 (prod) | rpm | AMA | AMA | medium |
| chip load | fz 0.09 / hex 0.088 (cons); fz 0.10 / hex 0.098 (prod) | mm | inside ONS 7/32 in band, over SAZ | ONS, SAZ, DAP | medium |
| feed | 3240 (cons) / 4000 (prod) | mm/min | fz x 2 x n | CALC | medium |
| plunge | start in air past the stock edge; if inside, ramp 3 deg | - | 0.5 mm is shallow | judgement | medium |
| ramp | 3 deg | deg | judgement | judgement | low |
| stepover | 2.4-3.0 (0.4-0.5 x D) | mm | power is 3 % of torque | CALC | medium |
| depth per pass | 0.5 (one pass) | mm | brief | brief | high |
| power at chosen point | 32 W (cons) / 40 W (prod) vs 1125 / 1250 W | W | u = 0.50 | GUE, CALC | medium |

Answers:

1. 18000-20000 rpm, 3240-4000 mm/min. The arcs of the small rings cut the real feed to 3600 at
   R 3 mm.
2. No. The tangential cut force is about 6 N (torque 19 N mm at r = 3 mm, CALC). Side pushers hold
   much more than that. The workholding does not set a lower limit than the tool.

Risks:

- Flatness: the pad must match the op 0 face. Use the same Z zero and the same tool length offset
  check. Feeds do not set this.
- A plunge into the stock with no ramp at 0.5 mm: acceptable, but the brief says "plunge 0.5 mm" with
  an unconfirmed centre cut.

## 13 Front bulk rough, 18 mm (op 14)

Planned: geodesic adaptive, ap up to 4.17 mm, ae 3.6 mm (factor 0.80), 3 levels, epoxy in the
lowest level.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 18000 | rpm | vendor chart rpm; torque not binding | FRD, CALC | medium |
| chip load | fz 0.125 / hex 0.10 (cons); fz 0.167 / hex 0.133 (prod) | mm | under FRD/ONS 0.20-0.33 | FRD, ONS | medium |
| feed | 4500 (cons) / 6000 (prod, the cap) | mm/min | fz x 2 x n | CALC | medium |
| plunge | no straight plunge into wood | - | non-centre-cutting, brazed | FRD | high |
| ramp angle / feed | <= 2 deg until measured, then <= 3 deg; helix pitch <= 1.3 mm until measured | deg | as brief 02 | CALC | medium |
| stepover | 3.6 (0.2 x D) | mm | T 67 % at the cap | CALC | medium |
| depth per pass | 4.17 (keep) | mm | < 0.25 x D, no depth reduction | FRD | medium |
| power at chosen point | 563 W (cons) / 750 W (prod) vs 1125 W | W | u = 0.50 | GUE, CALC | medium-low |

Answers:

1. **Yes.** At 4.17 x 3.6 the cap (6000 at 18000 rpm, fz 0.167, hex 0.133) needs 67 % of the rated
   torque. The cap binds, not the power.
2. As brief 02: no straight plunge; ramp <= 2 deg until measured; helix pitch <= 1.3 mm. The 175
   steep descents in the old sim are the main danger for this tool.
3. No feed drop is needed for the epoxy (judgement; no source found for a cured-epoxy chip load in
   this review). Keep the chip thick and avoid dwell: heat, not chip size, makes epoxy smear. The
   epoxy is only in the lowest level and under 2.4 mm deep.

Risks:

- Arcs with R under 8.3 mm do not reach 6000; the real chip falls in corners. Accept it.
- The rated rpm of the bit: as brief 01.

## 14 Front 3D rough, 6 mm (op 15)

Planned: ae 1.382 (0.23 x D, factor 0.842), ap 5.532 mm, rest stock, epoxy in places.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 18000 (cons) / 20000 (prod) | rpm | AMA | AMA | medium |
| chip load | fz 0.060 / hex 0.051 (cons); fz 0.095 / hex 0.080 (prod) | mm | between SAZ and ONS | SAZ, ONS, DAP | medium |
| feed | 2160 (cons) / 3800 (prod) | mm/min | fz x 2 x n; arcs cut it | CALC | medium |
| plunge | 720 if needed (centre cut confirmed) | mm/min | AMA | AMA | medium |
| ramp angle / feed | 3 deg, helix pitch 1-2 mm | deg | judgement | judgement | low |
| stepover | 1.38 (planned) or 1.8 with a shallower ap | mm | power is 12-19 % of torque | CALC | medium |
| depth per pass | 5.53 (planned, 0.92 x D) or 2.77 (alt) | mm | under 1 x D: no depth reduction | FRD | medium |
| power at chosen point | 138 W (cons) / 242 W (prod) vs 1125 / 1250 W | W | u = 0.50 | GUE, CALC | medium |

Options:

| Option | ap x ae | rpm | fz / hex | feed | T % |
|---|---|---|---|---|---|
| planned cons | 5.53 x 1.38 | 18000 | 0.060 / 0.051 | 2160 | 12 |
| planned prod | 5.53 x 1.38 | 20000 | 0.095 / 0.080 | 3800 | 19 |
| alt cons | 2.77 x 1.80 | 18000 | 0.060 / 0.055 | 2160 | 8 |
| alt prod | 2.77 x 1.80 | 20000 | 0.085 / 0.078 | 3400 | 11 |

Answers:

1. 18000-20000 rpm, 2160-3800 mm/min. The cut is light for the spindle (T <= 19 %).
2. Yes, if the finish quality matters more than the time. A depth of 2.77 mm halves the terrace
   height that the scallop ball meets later. Power permits it; the cost is about twice the levels.
   ae 1.8 (0.3 x D) puts back part of the time. This is a trade for the operator, not a limit.
3. The 4.6 mm entry bite (full slot) needs 44 % torque at the cons chip and 70 % at the prod chip.
   Both are acceptable. Deflection is small at this depth.

Risks:

- Epoxy and oak in one cut: keep the chip, no dwell.
- High air share: the real chip falls on short arcs; the cons point stays above the floor.

## 17 Outline profile with tabs, 6 mm (op 18)

Planned: full slot, 25 mm deep, 1.5 mm per level (17 levels), climb, 8 tabs, visible sides, end
grain at Y min and Y max.

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 18000 (cons) / 20000 (prod) | rpm | AMA | AMA | medium |
| chip load (1.5 mm levels) | 0.08 (cons) / 0.10 (prod) | mm | full slot, no thinning | ONS, SAZ | medium |
| feed (1.5 mm levels) | 2880 (cons) / 4000 (prod) | mm/min | fz x 2 x n | CALC | medium |
| plunge | none; ramp in the slot | - | - | judgement | medium |
| ramp angle / feed | 3 deg, at the cutting feed | deg | judgement | judgement | low |
| stepover | 6 (full slot); finish pass ae 0.3 | mm | - | brief, CALC | medium |
| depth per pass | 1.5 (keep) or 3.0 at 2160-2700 mm/min | mm | SAZ: 10-50 % of D for soft material | SAZ, CALC | medium |
| finish pass | ae 0.3 mm, full depth 25 mm, fz 0.115 (hex 0.05), 4140 mm/min at 18000 | mm | chip thinning factor 0.436 | DAP, CALC | medium-low |
| power at chosen point | 216 W (cons) / 300 W (prod) vs 1125 / 1250 W | W | u = 0.50 | GUE, CALC | medium |

Depth x feed pairs (full slot, 6 mm, CALC):

| ap | rpm | fz | feed | T % | Levels for 25 mm |
|---|---|---|---|---|---|
| 1.5 | 18000 / 20000 | 0.08 / 0.10 | 2880 / 4000 | 19 / 24 | 17 |
| 3.0 | 18000 | 0.06 / 0.075 | 2160 / 2700 | 29 / 36 | 9 |
| 6.0 (1 x D) | 18000 | 0.045 | 1620 | 43 | 5 |

Answers:

1. 18000-20000 rpm, 2880-4000 mm/min at 1.5 mm per level.
2. Deeper levels at a lower feed are possible: 3.0 mm at 2160-2700 or 6.0 mm at 1620 (torque 29-43 %).
   They take fewer levels but give the same or a lower MRR per minute and pack more chips in a
   25 mm slot. 1.5 mm is a good choice for a visible edge. The flute (25 mm) reaches the full depth;
   the shank equals the cut diameter, so the shank stays out of contact only if the slot is not
   narrower than 6.0 mm.
3. Yes. Rough the slot 0.3-0.5 mm outside the line, then make one finishing pass at ae 0.3 mm over
   the full depth (or in two steps of 12.5 mm) at 4140 mm/min, fz 0.115 (hex 0.05). It removes the
   burn and the deflection marks of 17 full-slot levels. A down-cut or compression bit for the top
   edge is better for fuzz; that is a tool choice, not a feed.

Risks:

- Chip packing and burning in a 25 mm deep, 6 mm slot with no air. A slot 1.3 x D wide (two offset
  passes) clears chips much better (judgement).
- End grain at Y min and Y max: break-out at the climb exit corners. Leave the finish pass for last.
- The tabs (2 mm) hold the part at the last levels: lateral force at 1.5 mm x fz 0.10 is about
  48 N (T 24 %, r 3 mm); tabs of 6 x 2 mm in oak hold it (judgement).

---

## Risks in order (all ops)

1. **Dangerous, op 1:** the planned 9 x 3.6 mm at the 6000 cap needs 108-144 % of the rated
   spindle torque at u = 0.50 J/mm^3. Use 9 x 2.7 at 6000 and 24000 rpm, or 9 x 3.6 at <= 4400.
2. **Dangerous, op 2:** the 14.5 mm entry and 13 mm link crossings need a chip near the rubbing
   floor to stay inside the torque. Fix the path.
3. **Dangerous, op 3:** the pin hole depth (30 mm) is over the flute (25 mm) and equal to the
   stick-out (30 mm). Collet nut contact and chip packing.
4. **Risky, ops 0, 1, 12, 14:** the 18 mm entry rule rests on two estimates. 3 deg and a 2 mm helix
   pitch fail for a bit with 0.5 mm protrusion and a 10 mm core. Measure the bit.
5. **Risky, op 5:** one fixed feed for a 0-10 mm full slot. Use levels or a depth-dependent feed.
6. **Risky, op 4:** the 0.5 s dwell burns the hole bottom.
7. **Risky, op 18:** burn and fuzz on visible sides without a finishing pass.
8. **Unknown, 18 mm bit:** its rated maximum rpm and its type (brazed or solid). The 24000 rpm
   options need a rating of 24000 rpm or more.

## What I could not source

- A measured specific cutting energy for white oak at 12 % moisture and chips of 0.05-0.15 mm. The
  design value (0.50 J/mm^3) comes from a router test on similar-density tropical species at a small
  chip. The Mandic et al. 2015 oak model (BioResources 10(3) 5489) gave values I could not reconcile
  with physical units, so I did not use it.
- A cited ramp angle for a centre-cutting 6 mm end mill in oak (3 deg is judgement).
- A cited peck rule for an end mill in wood (the Onsrud drill sheet has none).
- A cited chip load for cured epoxy.
- The rated rpm, maker and type of the 18 mm bit; its core and protrusion (ESTIMATES).
- The 6 mm bit's centre-cutting status.
- The machine frame stiffness: the deflection numbers cover only the tool.
