# Blind review C: finishing and small tools (nz-wanaka-21)

Reviewer C, Phase 2 of `planning/feeds_blind_review_plan_2026-11-03.md`. Date 2026-11-03.
Inputs: briefs 00, 07, 08, 15, 16; the machine file; `~/.config/rs_cam/tools/tapered_ball.toml`.
This review did not read rs_cam's Suggest, the operator's values or the rs_cam feeds code.

Calculation script: `planning/feeds_blind_review_2026-11-03/review_C_calc.py`. Run it with
`python3 -I`. It computes every chip, force, deflection, power and motion number below, and it
asserts the claims marked "(asserted)".

## Summary

| Op | Tool | rpm | Chip h_max (mm/tooth) | Feed (mm/min) | Depth x stepover | Plunge / ramp | Top risk |
|---|---|---|---|---|---|---|---|
| 6, 7 Lake pockets | DMM 2 mm 1F | 24000 (18000-24000) | 0.030 cons. / 0.045 prod. | 720 / 1080 (one feed for slot and 50 % rings) | 0.5 x 1.0 (now); 1.0 x 1.0 better | Plunge only if centre cutting is confirmed; else helix at 3 deg or less; plunge 250-400 mm/min | Centre cutting not known; shank size not known (3x deflection if 2 mm); burning if chip < 0.025 |
| 8 Antenna pocket | DMM 2 mm 1F | 24000 | 0.030 / 0.045 | 720 / 1080 | 1.0 x 1.0 (5 levels), not 5 x 1 | Linear ramp 3 deg; a helix with pitch 1.0 mm in a 4 mm slot is 9 deg or more | Ramp steeper than stated; chip packing in a 5 mm deep 4 mm slot |
| 16 Front rest R1 | R1 x 6 x 20 taper ball 2F | 24000 (20000-24000) | 0.030 / 0.050 | 1570 / 2620 at 24000 | 2.0 x 0.66 (now) is sane; 1.0 x 1.0 equal force per mm | Helix pitch 2.0 at r 0.6 = 28 deg: reduce pitch to 0.3 mm (5 deg) | Helix far steeper than 3 deg; real feed under 1200 mm/min drops the chip under the floor at 24000 |
| 17 Scallop finish | R1 x 6 x 20 taper ball 2F | 18000-20000 | 0.035 / 0.045 (after x1.155 axial thinning) | 1500 / 2200 commanded | scallop 0.1 mm (0.87 mm step on flats) | Lead arcs only; no plunge | The real feed varies 1000-3500 mm/min; chip falls under the floor in tight curves; the Z rate caps steep slopes at 2500-2900 mm/min |

The machine power never binds: the largest cut in this group takes under 40 W, and the spindle
gives 1125-1500 W at 18000-24000 rpm (asserted). The 6000 mm/min software cap never binds. The
24000 rpm limit binds the surface speed only: a 2 mm tool reaches 151 m/min at 24000 rpm. For a
small tool in wood the chip load matters, not the surface speed.

## Method

1. **Chip load.** The chip load is the feed per tooth, `fz = F / (n z)` (Onsrud Hard Wood chart,
   "Chip Load = Feed Rate / (RPM x # of cutting edges)", https://www.onsrud.com/images/Hard%20Wood.pdf,
   PDF sha256 f123d5f6...a69dc, the same file that `sources.toml` key `onsrud_hwood` records).
2. **Radial chip thinning.** When ae < 50 % of the cutting diameter D, the largest chip is
   `h_max = fz / RCTF` with `RCTF = D / (2 sqrt(ae (D - ae)))` (DAPRA, `sources.toml` key
   `dapra_rctf`, https://www.dapra.com/articles/radial-chip-thinning). At ae >= D/2, `h_max = fz`.
3. **Ball nose at shallow depth.** For ap < R, the cutting diameter is
   `De = 2 sqrt(R^2 - (R - ap)^2) = 2 sqrt(ap (2R - ap))`, and the feed per tooth is raised by
   `D / De` ("MMPTadj = (D x MMPT) / De"; IMCO, Adjustments for Ball Nose End Mills,
   https://cms.imcousa.com/wp-content/uploads/2021/01/Adjustments-for-Ball-Nose-Endmills.pdf).
4. **Tapered ball above the ball.** The taper half angle is
   `alpha = atan((6/2 - 1) / 20) = 5.71 deg`. The endmill.com.au table prints "R1mm x 6mm x 20mm 2F
   | 6 | R1 | 20 | 5.7" (https://www.endmill.com.au/2-flute-tapered-ball-nose-carbide-mill-for-3d-carving/),
   so 5.7 is the half angle. The ball meets the taper at `h_t = R (1 - sin alpha) = 0.90 mm`.
   Above it, `D(h) = 2 (R cos alpha + (h - h_t) tan alpha)`. D(0.5) = 1.73 mm, D(2.0) = 2.21 mm,
   D(5.4) = 2.89 mm (asserted).
5. **Projection derate.** IMCO prints a feed and speed factor for tool projection: "> 4 to 5 x D:
   x .80", "> 5 to 6 x D: x .70". Both tools in this group are past 6 x D. The chart stops there,
   so x 0.70 is the smallest printed factor. A lower factor is judgement.
6. **Rubbing floor.** 0.025 mm/tooth (Shaw, Metal Cutting Principles; `sources.toml` key
   `shaw_metal_cutting`). Under it the edge rubs and the oak burns.
7. **Force.** Curti et al. 2021 (Eur. J. Wood Wood Prod., doi:10.1007/s00107-021-01667-5),
   Table 5, helix 0, as quoted in `crates/rs_cam_core/src/material/force_line.rs`:
   `Fc / ap = Ks h + F_edge`, `Ks = Ks_n rho`, `F_edge = Int_n rho`. For white oak,
   rho = 0.68 x 1120 = 761.6 kg/m^3 (FPL Table 5-3a). The upper envelope over grain angle and
   milling mode gives Ks = 58.5 N/mm^2 and F_edge = 4.60 N/mm. The low case (down-milling along
   the grain) gives 42.7 N/mm^2 and 3.58 N/mm. The script uses the envelope. Curti measured mean
   chips of 0.041-0.091 mm; most chips here are smaller, so the line is extrapolated. "ap" here is
   the engaged edge length (for the ball, the arc plus the taper flank).
8. **Resultant force.** The script takes the radial force equal to the tangential force, so the
   resultant is 1.41 x Fc (judgement, conservative).
9. **Deflection.** A cantilever clamped at the collet, with a point load at the middle of the cut:
   `delta = integral (F (s - x_load)^2 / (E I(s))) ds`, which reduces to `F L^3 / (3 E I)` for a
   plain rod. E = 600 GPa (task brief). The fluted section bends as a rod of 0.8 x D (2 flutes) or
   0.7 x D (1 flute) (judgement). The shank bends at its full diameter.
10. **Bending stress.** `sigma = M / (pi d^3 / 32)` at each section. The allowable is 1000 MPa
    (judgement: a fatigue-safe value well under the rupture strength of micrograin carbide).
11. **Power.** `P = (n z / 60) (Ks ap ae fz + F_edge ap R psi)`, psi the engagement angle.
    Available power `P(n) = 1.5 kW x n / 24000` (brief 00).
12. **Motion limits.** grblHAL junction speed `v^2 = a delta_j sin(theta/2) / (1 - sin(theta/2))`,
    theta = 180 deg - turn angle, a = 1200 mm/s^2, delta_j = 0.1 mm. On a curve of radius r,
    `v = sqrt(a r)`. On a slope beta, the Z rate caps the path feed at `2500 / sin beta`.

### Base chip loads from vendor data

| Source | Tool | Material | Chip load (in/tooth) | mm/tooth | Note |
|---|---|---|---|---|---|
| Onsrud Hard Wood, series 64-000/65-000 | 1/16 in, 3/32 in | hard wood | .001-.003, .002-.004 | 0.025-0.076, 0.051-0.102 | 1 x D depth. Linear between 1/16 and 3/32 at 2.0 mm: 0.038-0.089 mm. The page does not name the series type. |
| Onsrud Hard Wood, series 77-100 | 1/8 in | hard wood | .003-.005 | 0.076-0.127 | 77-100 is a tapered ball series (`sources.toml` onsrud_hwood note). Scaled by D to 2 mm: 0.048-0.080 mm (judgement: linear scaling). |
| Amana Spektra 2D/3D carving chart v15, 3 flute ball nose (46280-K, 46284-K, 46286-K and others) | 1/32 in to 1.5 mm, and 3 mm to 1/8 in | "Wood, MDF, Sign-Foam" | .0015-.0025 | 0.038-0.064 | 18000 rpm, 1 x D. https://www.amanatool.com/pub/media/productattachments/Spektra-Coated-3D-Profiling-Feed-Chip-Load-Chart-v15.pdf |
| Shapeoko CNC A to Z | 1/16 in | "hard wood & metals" | - | 0.013-0.025 | Lumps wood with metal; the `sources.toml` note says it cannot speak for one species. Low bound only. |
| `project_rivmap_mono/manufacture/block/feeds_oak.toml` | flat_3 (3 mm); tbn_r1 | oak | - | 0.0319; 0.0249 | Output of another feeds generator, not a vendor chart. Shown for comparison only. |

No source gives data for the DMM DSP1f-D2.0-L08. A web search for "DSP1f" and "DMM single flute
D2.0 L08" found no product page (2026-11-03). Its centre-cutting state, its shank and its helix
are not known.

Result: a 2 mm tool in hard wood at a 1 x D depth and a short projection takes about
0.038-0.064 mm/tooth (Amana) to 0.038-0.089 mm/tooth (Onsrud, interpolated). The projection
derate x 0.70 gives 0.027-0.045 mm/tooth. That band sits just above the 0.025 mm rubbing floor.
**For these small tools, reduce the depth or the width to reduce the force. Do not reduce the chip
under 0.03 mm.**

---

## 07 Lake pockets, DMM 2 mm 1-flute (ops 6, 7)

### Values

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 24000 (18000-24000) | rpm | Hold the chip; the highest rpm gives the highest feed for a given chip. 18000 is the vendor reference speed. | Amana v15 "Operating RPM: 18,000"; machine max 24000 | medium |
| Chip load h_max | 0.030 conservative, 0.045 productive | mm/tooth | Vendor band 0.038-0.089 x 0.70 projection derate; keep >= 0.03 above the floor | Onsrud 64-000/65-000, Amana v15, IMCO projection chart, Shaw floor | medium-low (no DMM data) |
| Feed | 720 conservative, 1080 productive | mm/min | 24000 x 1 x fz | derived | medium |
| Feed, slot first ring | same as the rings | mm/min | At ae = 0.5 D, RCTF = 1, so h_max is the same in a slot and at 50 % (asserted). The slot doubles the mean force and the heat, not the peak force. | DAPRA | medium |
| Plunge (straight) | 250-400, only if centre cutting is confirmed | mm/min | 30-40 % of 720-1080 | Shapeoko CNC A to Z "30% to 40% of the feedrate for woods" | low |
| Ramp | helix or linear at 3 deg or less | deg | Centre cutting not known; a shallow ramp lets a non-centre-cutting end cut. No vendor angle exists for this tool. | judgement | low |
| Stepover | 1.0 (50 %) as planned | mm | No thinning at 50 %; the full first ring is a slot at the same chip | DAPRA | medium |
| Depth per pass | 0.5 as planned (safe); 1.0 better | mm | Force at 1.0 x slot is 9-10 N, deflection 18-21 um, stress 250-284 MPa | Curti 2021 force, cantilever | medium |
| Deflection estimate | 10-11 um at 0.5 mm (3.175 shank); 29-33 um if the shank is 2 mm | um | cantilever, stick-out 20 mm, flute 8 mm | script | low (shank and stick-out are estimates) |
| Power check | 2-8 W against 1500 W | W | Curti line x cutting speed | script | high (it does not bind) |
| Acceleration | 90 deg corner speed 1021 mm/min; full feed 1080 on a radius of 0.27 mm or more | mm/min | grblHAL junction formula | script | medium |

### Depth x stepover options (chip held, 24000 rpm, envelope force)

| ap x ae | Levels (op 6, 8.55 mm) | h_max | Feed cons. / prod. | Peak force | Deflection (3.175 / 2 mm shank) | Stress | Note |
|---|---|---|---|---|---|---|---|
| 0.5 x 1.0 (now) | 18 | 0.030 / 0.045 | 720 / 1080 | 4.5-5.1 N | 10-11 / 29-33 um | 129-147 MPa | Safe, slow. |
| 1.0 x 1.0 | 9 | 0.030 / 0.045 | 720 / 1080 | 9.0-10.2 N | 18-21 / 56-63 um | 250-284 MPa | Recommended. Half the levels. |
| 2.0 x 0.5 (25 %) | 5 | fz 0.030-0.045 gives h 0.026-0.039 | 720 / 1080 | 17-19 N | 30-34 / 97-109 um | 449-505 MPa | Needs a slot-free strategy (adaptive). Not with offset rings: their first ring is a 2 mm deep slot. |
| 2.0 x 2.0 (slot) | 5 | 0.030 / 0.045 | 720 / 1080 | 18-20 N | 31-36 / 101-115 um | 467-531 MPa | Possible, but the full slot packs chips in oak. Not advised. |

The 7.5 mm wood wall stays within the 8 mm flute. The margin is 0.5 mm. If the shank is 3.175 mm,
the step from 2 mm to 3.175 mm sits just above the flute. Measure the flute length before this op;
the shank must not touch the lake wall on the finishing contour.

### Answers

1. **Chip load, rpm and feed at 20 mm stick-out in oak.** 0.030-0.045 mm/tooth at 24000 rpm:
   720-1080 mm/min. The stick-out is 10 x D, past the IMCO chart. The cantilever gives only
   10-36 um at the planned and 2 x deeper cuts, so the stick-out does not bind if the shank is
   3.175 mm. If the shank is 2 mm, the deflection is about 3 x larger (asserted). Then stay at
   0.5-1.0 mm per level.
2. **One feed for the slot and the 50 % rings?** Yes. At ae = 50 % the chip is not thinned, so the
   largest chip is the same in both. The slot makes about twice the heat per revolution. If the
   first ring burns, lower the feed of the slot to about 80 % (judgement). Do not lower it under
   600 mm/min at 24000 rpm (0.025 mm floor).
3. **Is a straight 0.5 mm plunge per level safe?** Only if the tool is centre cutting. Look at the
   end with a loupe: the end edge must reach the axis. If it is centre cutting, plunge at
   250-400 mm/min. If it is not, a 0.5 mm plunge into fresh oak crushes the centre core and can
   break the tool. On the upper levels the plunge lands in the opening that op 2 made (air), so the
   risk applies to the lowest 1-2 mm, where each level plunges into fresh oak. A safe option
   without the ramp dressup: drill a start hole with op 2 at each lake (op 2 already opens the
   lakes 1.0 mm past the front), and start every level inside that opening.
4. **Depth x stepover pairs.** See the options table. 1.0 x 1.0 is the best pair for offset rings.
   Deeper levels need an adaptive strategy so that no full slot occurs.

---

## 08 Antenna pocket, DMM 2 mm 1-flute (op 8)

### Values

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 24000 | rpm | as brief 07 | as brief 07 | medium |
| Chip load h_max | 0.030 cons., 0.040 prod. | mm/tooth | as brief 07; productive lowered for a near-slot 5 mm deep (chip packing; judgement) | brief 07 sources | medium-low |
| Feed | 720 / 960 | mm/min | 24000 x 1 x fz | derived | medium |
| Plunge | not used (ramp entry) | - | - | - | - |
| Ramp | linear 3 deg or less; helix pitch 0.25 mm at radius 0.8-1.0 mm gives 2.3-2.8 deg | deg | `atan(pitch / (2 pi r))` (script) | derived | medium |
| Ramp feed | the cut feed (720-960) | mm/min | at 3 deg the end edge takes 5 % of the advance | judgement | low |
| Stepover | 1.0 (as planned) | mm | 4 mm pocket, 2 mm tool: one slot ring, then about 50 % | geometry | high |
| Depth per pass | 1.0 (5 levels); 0.5 is safe but slow | mm | 1.0 mm slot: 9-10 N, 18-21 um, 250-284 MPa | script | medium |
| Deflection estimate | 18-21 um (1.0 mm), 48-55 um (5 mm in one pass) | um | cantilever | script | low |
| Power check | 7-8 W (1.0 mm slot), 36-39 W (5 mm slot) | W | Curti line | script | high (does not bind) |

### Answers

1. **Chip load, rpm and feed in a near-slot.** 0.030-0.040 mm/tooth at 24000 rpm, 720-960 mm/min.
   The single flute has a large gullet, which helps the chip leave a narrow slot (Datron: the single
   flute gives good chip clearance, https://shop.datron.com/product/single-flute-end-mills; that page
   gives no wood value). Use air blast into the slot: chips that stay in a 4 mm slot are cut again
   and burn.
2. **Ramp angle.** 3 deg or less (judgement; no DMM data). The brief gives "ramp at 3 deg" and
   "helix pitch 1.0 mm". In a 4 mm pocket the helix radius is at most 1.0 mm. A 1.0 mm pitch on a
   1.0 mm radius is 9.0 deg (asserted), not 3 deg. Check which of the two values rs_cam applies.
   For a 3 deg helix at r = 1.0 mm the pitch is 0.33 mm. A linear ramp at 3 deg needs 9.5 mm of
   length for 0.5 mm and 19.1 mm for 1.0 mm.
3. **0.5 mm per level or deeper?** 1.0 mm per level at the same chip. One 5 mm pass is not advised:
   the bending stress at the flute root reaches 916-1043 MPa, which is at the allowable, and the
   chips pack in a 5 mm deep slot. A deeper level with a lower feed does not fix the chip packing.

---

## 15 Front rest R1 (valleys), R1 x 6 x 20 tapered ball (op 16)

### Values

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 24000 (20000-24000) | rpm | Hold the chip; the cut diameter is 2.0-2.2 mm | machine max | medium |
| Chip load h_max | 0.030 cons., 0.050 prod. | mm/tooth | Vendor 0.038-0.080 (Amana 3F carving; Onsrud 77-100 scaled) x 0.70 projection derate; the taper keeps the tool stiff, so the derate is generous | Amana v15, Onsrud 77-100, IMCO | medium |
| Feed per tooth fz | 0.033 / 0.055 (with RCTF 1.093 at ae 0.66 on De 2.21) | mm/tooth | h_max x RCTF | DAPRA | medium |
| Feed | 1570 cons., 2620 prod. at 24000 | mm/min | 24000 x 2 x fz | derived | medium |
| Plunge | 500-800 | mm/min | 30-40 % of the cut feed; the ball cuts at the centre at near-zero speed, so keep the plunge short | Shapeoko CNC A to Z | low |
| Ramp (helix) | 5 deg or less: pitch 0.3 mm at radius 0.6 mm | deg | The planned pitch 2.0 mm at radius 0.3 x 2 = 0.6 mm gives 27.9 deg (asserted), not 3 deg | derived | high (geometry) |
| Stepover | 0.66 (as planned) to 1.0 | mm | see options | script | medium |
| Depth per pass | 2.0 (as planned) or 1.0 | mm | see options | script | medium |
| Deflection estimate | 15-17 um at 30 mm, 18-21 um at 35 mm (ap 2.0, ae 0.66) | um | tapered cantilever; the taper reaches 6 mm at 20 mm | script | medium-low |
| Bending stress | 84-100 MPa | MPa | largest at about 6 mm above the tip | script | medium-low |
| Power check | 15-16 W against 1500 W | W | Curti line | script | high (does not bind) |
| Acceleration | Real feed under 1200 mm/min gives a chip under 0.025 mm at 24000 rpm | mm/min | 0.025 x 2 x 24000 | derived | medium |

### Depth x stepover options (24000 rpm, envelope force, stick-out 30 / 35 mm)

| ap x ae | De | RCTF | h 0.03: feed | h 0.05: feed | Peak force | Deflection 30 / 35 mm | Note |
|---|---|---|---|---|---|---|---|
| 2.0 x 0.66 (now) | 2.21 | 1.093 | 1573 | 2622 | 23-27 N | 15-17 / 18-21 um | Sane. Fewest Z levels. |
| 1.5 x 0.8 | 2.11 | 1.031 | 1484 | 2473 | 19-22 N | 13-15 / 15-18 um | Middle. |
| 1.0 x 1.0 | 2.01 | 1.000 | 1440 | 2400 | 14-17 N | 10-12 / 12-14 um | Twice the levels; a finer terrace for the finish. |
| 0.5 x 0.87 | 1.73 | 1.000 | 1440 | 2400 | 9-11 N | 7-8 / 8-10 um | Like a finish; too many levels for a rest op. |
| 1.0 x 2.0 (full slot entry into a valley) | 2.01 | 1.000 | 1440 | 2400 | 14-17 N | 10-12 / 12-14 um | The power doubles (22-25 W); the chip packs. |

### Answers

1. **Is the R1 tapered ball sane as a rest roughing tool, and at what load?** Yes, at a moderate
   load. The taper makes it much stiffer than a 2 mm straight tool at 30 mm: the cantilever gives
   15-21 um and 84-100 MPa at 2.0 x 0.66 (asserted under 40 um). Run h_max 0.03-0.05 mm/tooth
   at 24000 rpm, 1570-2620 mm/min. The limits are not breakage. They are:
   - the chip packs in the small flute when the cut is buried 2 mm in a narrow valley,
   - the real feed falls on short contours, so the chip falls toward the floor and the oak burns,
   - the ball centre on each Z-level floor cuts at near-zero speed.
   The air share is high (301 m of cut against 676 m of rapids in a similar variant). That costs
   time, not load.
2. **Chip load at the real cut diameter (2.0-2.3 mm at 1-2.5 mm deep).** For ap > 0.9 mm there is
   no axial thinning; De = 2.01-2.31 mm. Use h_max 0.03-0.05 mm. With ae = 0.66 mm the radial
   thinning factor is 1.093 on De 2.21, so command fz = 0.033-0.055 mm/tooth.
3. **Depth x stepover pairs.** See the table. 2.0 x 0.66 and 1.0 x 1.0 both hold the chip at one
   feed. 2.0 x 0.66 has the larger force (23-27 N against 14-17 N) but half the levels. Both are
   far from a mechanical limit. Choose 1.0 x 1.0 if burns show on the terraces or if the 0.5 mm
   stock left is not even.

---

## 16 Front scallop finish, R1 x 6 x 20 tapered ball (op 17)

### Values

| Field | Value or range | Unit | Basis | Source | Confidence |
|---|---|---|---|---|---|
| rpm | 18000 cons., 20000 prod. | rpm | Lower rpm brings the commanded feed for a given chip down to the speed that the machine reaches on this path | script motion limits | medium |
| Chip load h_max | 0.035 cons., 0.045 prod. | mm/tooth | Amana carving 0.038-0.064; finish at the lower end; >= 0.03 above the floor | Amana v15, Shaw | medium |
| Feed per tooth fz | 0.040 / 0.052 | mm/tooth | h x D/De = h x 2.0/1.732 = h x 1.155 at 0.5 mm deep; ae 0.87 = 50 % of De, so RCTF = 1 | IMCO, DAPRA | medium |
| Feed (commanded) | 1500 at 18000 (h 0.036); 2200 at 20000 (h 0.048) | mm/min | n x 2 x fz | derived | medium |
| Plunge | none (lead arcs) | - | - | - | - |
| Lead / link | lead arcs r 2.0 mm reach 2940 mm/min; fine | mm/min | `sqrt(a r)` | script | medium |
| Stepover | 0.87 on flats (scallop 0.1 mm) | mm | `2 sqrt(2 R s - s^2)` | geometry (asserted) | high |
| Depth per pass | 0.5 nominal stock | mm | brief | - | - |
| Deflection estimate | 7-10 um at 0.5 mm stock | um | cantilever | script | medium-low |
| Deflection, deep bite 5.4 mm | 22-26 um (30 mm), 27-33 um (35 mm); stress 130-157 MPa | um | cantilever, full-width 5.4 mm bite | script (asserted under 40 um) | low |
| Power check | under 10 W | W | Curti line | script | high (does not bind) |

### The real feed on this path (grblHAL, a = 1200 mm/s^2, junction deviation 0.1 mm)

| Feature | Speed limit |
|---|---|
| Turn of 10 / 20 / 30 / 45 / 90 deg at a vertex | 10635 / 5292 / 3499 / 2290 / 1021 mm/min |
| Curve radius 0.5 / 1 / 2 / 5 / 10 mm | 1470 / 2078 / 2939 / 4648 / 6573 mm/min |
| Slope 30 / 45 / 60 / 75 / 90 deg (Z rate 2500 mm/min) | 5000 / 3536 / 2887 / 2588 / 2500 mm/min |
| Distance to reach 2000 / 3000 mm/min from rest | 0.46 / 1.04 mm |

The Z axis also has a lower acceleration (550 mm/s^2). Each change of slope direction on a ridge
or a valley slows the path more. The script does not model that.

Chip at the real feed (2 flutes, axial factor 1.155):

| Real feed (mm/min) | Chip at 18000 rpm | Chip at 24000 rpm |
|---|---|---|
| 1000 | 0.024 (under the floor) | 0.018 (under the floor) |
| 1500 | 0.036 | 0.027 |
| 2000 | 0.048 | 0.036 |
| 2500 | 0.060 | 0.045 |
| 3000 | 0.072 | 0.054 |

### Answers

1. **Chip load at 0.5 mm deep (De 1.73 mm) in oak and epoxy.** h_max 0.035-0.045 mm/tooth.
   Command fz 0.040-0.052 mm/tooth (axial thinning x 1.155). No source gives an epoxy chip load.
   Use the same chip and keep the feed steady (judgement): a thin chip and a dwell heat the epoxy,
   which then smears at the oak/epoxy edge. The feed marks are negligible: `fz^2 / (8 R)` is
   under 0.001 mm.
2. **Useful commanded feed and the rpm for it.** Command 1500-2200 mm/min, not more. Above about
   2500 mm/min the Z rate caps every slope over 60 deg, and curves under 2 mm radius cap the XY
   feed at 2000-2900 mm/min. A higher command only lowers the real chip relative to the plan.
   Set rpm from the feed that the path really reaches: 18000 rpm at 1500 mm/min gives 0.036 mm;
   20000 rpm at 2200 mm/min gives 0.048 mm. At 24000 rpm any part of the path under 1300 mm/min
   cuts under the 0.025 mm floor. Before a final value, read the real-feed distribution of the op
   from the simulation and pick the rpm for its median.
3. **Safe load if the ball still meets a deep bite (up to 5.4 mm).** The tool does not break by
   bending: at h 0.02-0.04 mm the force is 49-59 N, the deflection 22-33 um and the stress
   130-157 MPa (asserted). The Onsrud depth rule ("3 x D Reduce chip load by 50%") puts a 2.7 x D
   bite at about half the chip, which is at the 0.025 mm floor. So a finishing feed cannot be both
   safe for the bite and right for the surface. The risks of a bite are chip packing, burning and
   a gouge, not a broken tool. Op 16 must remove the bites. Re-measure the 13.7 % "over 1.08 mm"
   finding after op 16 before the finish runs.

---

## Risks (largest first)

1. **DMM centre cutting not known (ops 6, 7).** A straight 0.5 mm plunge into fresh oak with a
   non-centre-cutting end crushes the core and can snap a 2 mm tool. Confirm the end geometry
   before the run, or start each level in the opening that op 2 made.
2. **Helix angles much steeper than the stated 3 deg.** Op 16: pitch 2.0 mm at radius 0.6 mm is
   27.9 deg. Op 8: pitch 1.0 mm at radius 1.0 mm or less is 9 deg or more. If rs_cam applies the
   pitch, the entry is close to a plunge. Check which value rs_cam applies.
3. **Chip under the rubbing floor where the real feed drops (ops 16, 17).** At 24000 rpm a 2-flute
   tool needs 1200 mm/min for 0.025 mm/tooth. The scallop path cannot keep that in tight curves.
   This gives burn marks on oak and smear on epoxy.
4. **DMM shank and stick-out are estimates.** If the shank is 2 mm and not 3.175 mm, the deflection
   is about 3 x larger (29-33 um at 0.5 mm, 101-115 um at a 2 mm slot). Measure both.
5. **The 0.5 mm flute margin on the lake wall (ops 6, 7).** The 7.5 mm wood wall against an 8 mm
   flute leaves 0.5 mm before the shank. A longer stick-out does not help; a shorter flute does
   not fit.
6. **Deep bites in the scallop finish.** Mechanically safe in the model, but a 2 mm flute buried
   5.4 mm packs chips and burns. Op 16 must remove them.
7. **Force model extrapolated.** Curti measured mean chips of 0.041-0.091 mm. The chips here are
   0.02-0.05 mm, so the forces are extrapolated. The edge term (4.6 N per mm of edge) is most of
   the force at these chips. The deflection numbers are estimates; their ratios between options are
   more reliable than their absolute values.

## What I could not source

- Any DMM data for DSP1f-D2.0-L08: chip load, centre cutting, shank, helix, ramp angle.
- A vendor chip load for a 2-flute R1 tapered ball in hardwood. endmill.com.au prints the geometry
  but no feeds ("This calculator can only calculate straight cutters, not V bits or tapered bits",
  https://www.endmill.com.au/blog/cnc-router-feeds-and-speeds-the-adams-guide/). The band comes
  from Amana 3-flute carving bits and the Onsrud 77-100 1/8 in row, scaled.
- An epoxy chip load or a heat limit for epoxy.
- A projection derate past 6 x D (IMCO stops at x 0.70 for 5-6 x D).
- A ramp angle rule for a 2 mm single flute; 3 deg is judgement.
- The bending allowable for carbide (1000 MPa) and the core-diameter factors (0.7, 0.8) are
  judgement.
