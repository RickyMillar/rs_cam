# G10: Entry parameters (ramp angle, helix, plunge, ramp feed, clearance)

Status: Phase 2 (trend) done 2026-09-25. No number moves. The rulings in §3
come first; nothing lands before them.

Inputs: `fetch/G10/statements.json` (243: 238 confirmed, 3 wrong and 2
grade_wrong, corrected) and `sources.json` (60, 43 stored), merged by
`scripts/g10_verified.py` (notes: `fetch/G10/FETCH_NOTES.md`);
`INVENTORY_G10.md` (master `40f2b744`) and `g10_inventory_cells.csv`; the FM1
CSV (the working-tree copy equals `HEAD` in every column read here); the
engine constants, read from the Rust source by regex.

Script: `scripts/trend_g10.py` (python3, stdlib, read-only). It prints every
table in §1 (T0-T9) and writes `fetch/G10/trend_g10.out`. Every number that
is not a printed value is **derived** by that script. No vendor prints it.

## 0. The gap

Summary of `INVENTORY_G10.md`; it holds the file:line detail.

| Parameter | Repo value today | Where | Source |
|---|---|---|---|
| Ramp angle | dressup 3.0 deg; Adaptive3d 10.0 deg; CLI 3.0 deg (both); Pencil cap 12.0 deg | `compute/config.rs:1036`, `compute/operation_configs.rs:832`, `cli/job.rs:1116`, `:1356`, `finish/pencil/emission.rs:70` | none |
| Helix radius (path) | dressup 2.0 mm absolute; Adaptive3d 0.3 x D; CLI Adaptive3d 0.4 x D | `compute/config.rs:1037`, `operation_configs.rs:836`, `cli/job.rs:1351` | none |
| Helix pitch | dressup 1.0 mm; Adaptive3d 2.0 mm; CLI 1.0 mm | `compute/config.rs:1038`, `operation_configs.rs:840`, `cli/job.rs:1352` | none |
| Plunge | wood 1000 / h, sheet 900 / h at 6 mm, linear in D; ball and tapered-ball cap 150 per mm of tip; plunge <= feed | `material/mod.rs:1174`, `feeds/mod.rs:2558`, `feeds/suggest/invariants.rs:237` | none printed (a port of the Shapeoko reference calculator; the cap cites FSWizard / GWizard in a comment) |
| Drill plunge (G6) | axial chip = side chip / Z | `feeds/extrapolation/drill.rs:81` | Amana Spektra v24 (ruling B5) |
| Ramp feed | `ramp_feed_rate: None` on 22 op configs; by default the dressup runs 0.5 x plunge (derived), Adaptive3d the plunge | `9887735d` | none |
| Entry clearance | `entry_clearance_mm` 0.5 mm; 2.0 mm over a nominal stock top | `dressup/entry_descent.rs:356`, `:131` | operator ruling 2026-09-25 |
| Entry style | Suggest rewrites Adaptive3d Plunge to Ramp or Helix; Roughing role gets Ramp; `PreferHelix` forces Helix on 2D Adaptive | `feeds/suggest/adaptive_entry.rs:62`, `compute/config.rs:1099`, `:1193` | none |

No value reads the tool kind, the flute count or the material, except the
plunge (material, D, ball tip). No tool kind is refused an entry style.

The cells (T8, ok / all, from `g10_inventory_cells.csv`):

```
  kind                   E1       E2       E3      E4a      E4b       E5       E6       E7   ok total
  EndMill           40/40     8/8      8/8      0/24    40/64     8/8     16/24    16/16     136
  BullNose          40/40     8/8      8/8      0/24    34/64     6/8     12/24     0/16     108
  BallNose          28/40     6/8      6/8      0/24    38/64     4/8     16/24     0/16     98
  TaperedBallNose   40/40     8/8      8/8      0/24    64/64     8/8     24/24     0/16     152
  VBit              20/40     0/8      0/8     18/24     0/64     6/8      4/24     0/16     48
  ok per class: E1 168, E2 30, E3 30, E4a 18, E4b 176, E5 32, E6 72, E7 16; all 542
```

E1 = Face, Pocket, Profile, Rest, Zigzag (dressup ramp 3 deg). E2 = Adaptive
(dressup helix 2.0 mm / 1.0). E3 = Adaptive3d (Suggest rewrite to Ramp 10 deg
in FM1). E4-E6 = no default entry. E7 = drill. Every one of the 526 ok
non-drill cells ships a plunge; 228 ok cells (E1-E3) ship a ramp or a helix.

## 1. The trend

### 1.1 Ramp angle (T1)

Every ramp-angle statement is grade c. Wood or router vendor statements at
grade a or b: **0**. The full list (21 rows) is T1 in `trend_g10.out`.

| Source (statements) | Tool | Z | Centre cut | Printed deg |
|---|---|---|---|---|
| Harvey, Helical (5) | end mill, general | - | - | recommend 3-10 (soft / non-ferrous), 1-3 (hard / ferrous) |
| SGS matrix `-mx-47`, `-mx-43` | S-Carb | 2, 3 | yes | max 90 |
| SGS `-mx-upcut` / `-mx-compression` / `-mx-downcut` | router series 21 / 25 / 22 | (2) | yes | max 90 / 5 / "–" (no legend) |
| SGS `-mx-56b` | Turbo-Carb ball | 2 | yes | max 25 |
| SGS `-mx-series7`, `-mx-66`, `-mx-77` | multi-flute | 4-7 | yes / no | max 1 |
| SGS `-mx-z5`, `-z5-ramp5` | Z5 rougher | 5 | no | max 7 (matrix) against 5 (chart) |
| CNCCookbook `hobby-024`, `-025` | end mill, metal | - | - | common 1.5-2.5; OSG (second hand) 10-20 |
| Carbide 3D forum `hobby-012` | any (user post) | - | - | Carbide Create default 20 |

Repo: 3 deg is the low end and 10 deg the top of the Harvey "soft /
non-ferrous" range. 3 deg is below the SGS compression-router maximum
(5 deg); 10 deg is above it. The SGS router rows are colour-coded for
non-ferrous metal only (verifier a). No statement prints a ramp angle for a
ball, tapered ball or V-bit in wood.

### 1.2 Helix size in one frame (T2)

Frames: bore diameter B, path (tool-centre) diameter P, path radius r.
B = P + D and r = (B - D) / 2. A flat tool leaves no core when r <= D/2
(P <= D, B <= 2D). With a corner radius rc the limit is r <= D/2 - rc; this
is IMCO's "(tool diameter x 2) - (corner radius x 2)" for the bore.

```
  statement                        frame      bound    r/D         grade  reading
  g10-metal-harvey-te-helix-dia    ambiguous  min      0.05-0.10   c      '>110-120%' read as BORE
  g10-metal-helical-gb-helix-dia   ambiguous  min      0.55-0.60   c      same words read as PATH diameter (leaves a core)
  g10-metal-imco-bore-2d-2r        bore       max      0.50        c      B = 2D - 2rc; r = D/2 - rc
  g10-metal-sandvik-max-hole       bore       max      0.50        c      max hole 2 x D3 (inserts)
  g10-hobby-018                    path       max      0.50        c      Fusion: helix diameter must not exceed D
  g10-hobby-019                    path       example  0.40        c      Fusion figure 0.8 x Dia (good) vs 1.8 x Dia (boss)
```

The Harvey / Helical words (">110-120% of the cutter diameter") are
ambiguous. The path reading gives r = 0.55-0.60 D, which leaves a core of
0.1-0.2 D on a flat tool. That contradicts IMCO, Sandvik and Fusion.
Only the bore reading (r = 0.05-0.10 D, a minimum) agrees with the geometry.
This is an inference; Harvey does not say it.

Repo defaults per matrix size (path radius / D, derived):

```
  kind                  D no-core r/D |  A3d 0.3D dressup 2.0mm CLI3d 0.4D | centre pip height (mm) A3d / dressup / CLI3d
  EndMill           3.175       0.500 |     0.300         0.630      0.400 | 0 / core 0.82 / 0
  EndMill           6.000       0.500 |     0.300         0.333      0.400 | 0 / 0 / 0
  BullNose          3.175       0.350 |     0.300         0.630      0.400 | 0 / core 1.78 / 0.027
  BullNose          6.000       0.350 |     0.300         0.333      0.400 | 0 / 0 / 0.051
  BallNose          3.175       0.000 |     0.300         0.630      0.400 | 0.317 / core 0.82 / 0.635
  BallNose          6.000       0.000 |     0.300         0.333      0.400 | 0.600 / 0.764 / 1.200
  TaperedBallNose   3.175       0.000 |     0.300         0.630      0.400 | 0.317 / core 0.82 / 0.635
  TaperedBallNose   6.000       0.000 |     0.300         0.333      0.400 | 0.600 / 0.764 / 1.200
  VBit              6.350       0.000 |     0.300         0.315      0.400 | 3.300 / 3.464 / 4.399
  VBit             12.700       0.000 |     0.300         0.157      0.400 | 6.599 / 3.464 / 8.799
```

"core X" is the diameter of the stock column that the helix leaves standing
(2 (r - flat bottom radius)). Ball and tapered ball: pip = R - sqrt(R^2 - r^2).
V-bit: pip = r / tan 30 deg. Caution: Adaptive3d multiplies the factor by the
envelope diameter; for the tapered ball the FM1 diameter is the tip, so its
real Adaptive3d radius is larger than the table shows.

### 1.3 Helix pitch and helix angle (T3)

Helix angle = atan(pitch / (2 pi r_path)), derived:

```
  arm                    r rule     pitch | D=3.175  D=6      D=6.35   D=12.7
  A3d default            0.3D         2.0 |  18.48    10.03     9.49     4.78
  A3d, pitch 1           0.3D         1.0 |   9.49     5.05     4.78     2.39
  dressup default        2.0 mm       1.0 |   4.55     4.55     4.55     4.55
  dressup, pitch 2       2.0 mm       2.0 |   9.04     9.04     9.04     9.04
  CLI Adaptive3d         0.4D         1.0 |   7.14     3.79     3.59     1.79
  CLI Adaptive3d, p 2    0.4D         2.0 |  14.07     7.55     7.14     3.59
```

Printed helix angles (all metal, grade c): IMCO IPT/C 7-13 flute 0.5 deg;
APT/C 5 3 deg; M5xx-M9xx 1-2.5 deg; M2xx 3-5 deg. Harvey / Helical ramp
3-10 deg soft. Sandvik prints a pitch limit (<= max ap), no angle. No wood,
router or hobby source prints a pitch or a helix angle. The Adaptive3d default
on a 3.175 mm tool (18.5 deg) is steeper than every printed range except the
SGS centre-cutting rows (25 deg ball, 90 deg 2-3 flute).

### 1.4 Plunge feed (T4)

Printed fractions (plunge / side feed, derived from printed operands):

```
  source                                   family            what     gr   n fraction      median
  sienci_feeds_speeds_metric               flat_end_mill     plunge   a   60 0.500-0.505    0.500
  wood_idc_feeds_speeds                    flat_end_mill     plunge   a    5 0.047-0.500    0.429   (up 0.50; down 0.30, 0.43; 1 in surfacing 0.047)
  carbide3d_s3_feeds_250                   flat_end_mill     plunge   b    5 0.375-0.533    0.492   (one row for square and ball)
  carbide3d_nomad883_feeds_125             flat_end_mill     plunge   b    5 0.240-0.444    0.418
  wood_amana_spektra_plunge_v24            flat_end_mill     RampDown a    3 0.333-0.500    0.333   (rule 1/Z)
  wood_amana_compression_v8                compression       RampDown a    4 0.334-1.000    0.500   (1/Z, Z 1-3)
  wood_amana_corner_radius_plunge          bull_nose         RampDown b    1 0.500          0.500   (rule, Z 2)
  wood_idc_feeds_speeds                    bull_nose         plunge   a    1 0.188          0.188   (1 in bowl bit)
  sienci_feeds_speeds_metric               ball_nose         plunge   a   34 0.500-0.503    0.500
  wood_amana_ball_nose_v7                  ball_nose         RampDown b    1 0.500          0.500   (rule)
  wood_idc_feeds_speeds                    ball_nose         plunge   a    2 0.250-0.429    0.339
  sienci_feeds_speeds_metric               tapered_ball_nose plunge   a    8 0.334-0.504    0.500
  wood_idc_feeds_speeds                    tapered_ball_nose plunge   a    1 0.417          0.417
  sienci_feeds_speeds_metric               v_bit             plunge   a    4 0.330-0.335    0.333
  wood_idc_feeds_speeds                    v_bit             plunge   a    4 0.333-0.571    0.465   (60 deg 0.333)
  wood_amana_insert_vgroove_v16            v_bit             RampDown a    3 0.500-1.000    0.500
  metal: Garr 0.50; SGS 0.25 x slot feed; Harvey chamfer 0.40-0.50, engraver 0.50; CNCCookbook slot feed / Z (all c)
  prose: ToolsToday about 0.5; ShopBot 0.294 (0.5 in/s against 1.7 in/s); Onsrud composite 0.5 (all c)
```

Absolute (wood): PreciseBits bull 3F 3.175 mm 1905 / 1270 / 1016 mm/min by
Janka (600 / 400 / 320 per mm of D); PreciseBits V-tip 1016-6350 (1/4 in
shank) and 127-635 (micro). Sienci flat 1.587-6.35 mm 430-1370 mm/min
(128-428 per mm). Repo base: 167 / h (wood) and 150 / h (sheet) per mm.

Repo against the printed median for the tool kind (ok, non-drill cells):

```
  kind                  D   n repo frac     ratio       Pocket frac   Pocket ratio
  EndMill           3.175  60 0.090-0.389   0.18-0.78   0.093-0.145   0.19-0.29
  EndMill           6.000  60 0.171-0.742   0.34-1.48   0.171-0.250   0.34-0.50
  BullNose          3.175  54 0.090-0.145   0.18-0.29   0.093-0.145   0.19-0.29
  BullNose          6.000  54 0.171-0.426   0.34-0.85   0.176-0.426   0.35-0.85
  BallNose          3.175  49 0.093-0.542   0.19-1.08   0.093-0.119   0.19-0.24
  BallNose          6.000  49 0.175-0.721   0.35-1.44   0.176-0.225   0.35-0.45
  TaperedBallNose   3.175  76 0.090-0.132   0.18-0.26   0.103-0.132   0.21-0.26
  TaperedBallNose   6.000  76 0.171-0.326   0.34-0.65   0.254-0.326   0.51-0.65
  VBit              6.350  20 0.271-0.780   0.81-2.33   0.712-0.780   2.13-2.33
  VBit             12.700  28 0.542-1.000   1.62-2.99   1.000         2.99
  printed median used: EndMill 0.500, BullNose 0.500 (Amana rule), BallNose 0.500, TaperedBallNose 0.500, VBit 0.335
  all ok non-drill cells: n=526, ratio median 0.35, range 0.18-2.99
```

The per-material rows are in `trend_g10.out` (T4b). The repo plunge is one
number per tool, size and material; the fraction moves with the op's feed.

The G6 drill claim against the milling plunge on the same tool (T4d):

```
       D material          Drill feed=plunge Drill RPM Pocket plunge Pocket feed Pocket F/Z Drill/Pocket plunge
   3.175 softwood                       1422     14000           529        3657       1828                2.69
   3.175 hardwood                       1422     14000           371        3657       1828                3.83
   3.175 mdf                            1778     14000           373        3999       2000                4.77
   3.175 plywood_hardwood               1422     14000           360        3657       1828                3.95
   6.000 softwood                       1778     14000          1000        3999       2000                1.78
   6.000 hardwood                       1778     14000           702        3999       2000                2.53
   6.000 mdf                            2133     14000           706        3999       2000                3.02
   6.000 plywood_hardwood               1778     14000           682        3999       2000                2.61
```

On the 6 mm flat end mill in hardwood the engine drills at 1778 mm/min and
plunges a pocket at 702 mm/min. The G6 form at the pocket RPM would give
2000 (F / Z).

### 1.5 Ramp feed (T5)

Printed rules: Amana "To find Ramp Down: Feed Rate IPM / # of flutes"
(meaning not defined); SGS "Use slotting speeds and feeds for ramp angles of
1° to 2°" and "Reduce feed to 25% when ramp angles approach 6°"; Sandvik
linear ramp 75 % (inserts); IMCO 1.0 x slot feed (M series), 1.25-1.6 x IPT
(IPT/C); Harvey straight and roll-in entries at 50 %. CAM conventions (grade
c): Carbide Create Pro max(F/3, plunge); Vectric and its tool database ramp
at the plunge rate; Fusion has a separate field with no value.

**The approved design, checked.** Ramp feed = min(F, a_z x n x Z / tan theta).
The G6 axial chip is a_z = f_z,side / Z (Amana). The side feed is
F = f_z,side x n x Z. Then:

- a_z x n x Z = f_z,side x n = F / Z.
- The second term is F / (Z tan theta).
- It is below F only when tan theta > 1/Z: theta > 26.57 deg for Z = 2,
  theta > 18.43 deg for Z = 3.
- Where the machine cap (4000) holds F below f_z,side x n x Z, the threshold
  angle is higher still.

The derivation is correct. One refinement: the vertical rate on a path
feed is F_ramp x sin theta, so holding it at F / Z gives F / (Z sin theta).
Tan and sin differ by cos theta (0.14 % at 3 deg, 1.5 % at 10 deg). This
does not change a result.

```
  theta (deg) arm                         1/(Z tan) Z=2    Z=3  SGS (metal)
         2.00 SGS 'slotting feed'                 14.32   9.55         1.00
         3.00 dressup ramp                         9.54   6.36            -
         4.55 dressup helix 2.0 mm / 1.0           6.28   4.19            -
         6.00 SGS '25 %'                           4.76   3.17         0.25
        10.00 A3d ramp                             2.84   1.89            -
        18.48 A3d helix, D 3.175                   1.50   1.00            -
        20.00 Carbide Create (user post)           1.37   0.92            -
```

Values > 1 mean that the approved form ships the full cutting feed. The SGS
vertical rate (fraction x sin theta, of the slot feed) is 0.017 at 1 deg,
0.035 at 2 deg and 0.026 at 6 deg. The SGS plunge is 0.25. So SGS holds a
ramp far below its own plunge rate: its rule is not an axial-chip rule, and
the approved form cannot reproduce it.

Per FM1 cell (ok, E1-E3; T5c, condensed):

```
  kind                  D cls basis        n  theta term/F      ships F F mm/min    literal plunge today (derived)
  EndMill           3.175 E1  G6 chip     20   3.00 3.9-42.7     20/20  954-4000    360-529        180-264
  EndMill           3.175 E2  G6 chip      4   4.55 6.3-7.2       4/4   3658-4000   360-529        180-264
  EndMill           3.175 E3  G6 chip      4  10.00 2.8-3.2       4/4   3658-4000   360-529        360-529
  EndMill           6.000 E1  G6 chip     20   3.00 4.8-9.5      20/20  3165-4000   682-1000       341-500
  EndMill           6.000 E2  G6 chip      4   4.55 7.2-7.2       4/4   4000        682-1000       341-500
  EndMill           6.000 E3  G6 chip      4  10.00 2.0-3.2       4/4   4000        682-1000       682-1000
  BullNose          3.175 E3  plunge/tan   4  10.00 0.5-0.8       0/4   3658-4000   360-529        360-529
  BullNose          6.000 E3  plunge/tan   4  10.00 1.0-2.4       3/4   1600-4000   682-1000       682-1000
  BallNose          3.175 E3  plunge/tan   3  10.00 0.5-0.7       0/3   3658-4000   371-476        371-476
  BallNose          6.000 E3  plunge/tan   3  10.00 1.0-1.3       2/3   4000        702-900        702-900
  TaperedBallNose   3.175 E3  plunge/tan   4  10.00 0.6-0.7       0/4   3064-3605   360-476        360-476
  (the other 13 plunge/tan rows: E1, E2, and E3 on the 6 mm tapered ball; term/F 1.2-19.1; F ships on all)
  G6 chip: 56 cells; the second term binds on 0.
  plunge/tan: 172 cells; the second term binds on 13 (the E3 rows above).
```

"literal plunge" is the fallback read as "ramp feed = plunge rate, as today".
"plunge/tan" is the fallback read as the plunge rate being the vertical
limit: min(F, plunge / tan theta). "today" is 0.5 x plunge (dressup) or the
plunge (Adaptive3d). Caution: a commanded feed is not a time. The rivmap100
arms show that the gain is accel-bound (INVENTORY_G10 3.5).

### 1.6 Entry style and no-plunge rules (T6)

| Family | Statement | Source | Grade |
|---|---|---|---|
| Down-cut spiral | "Downcut tools CANNOT be used to plunge straight into wood and should be ramped into the part."; "Never plunge straight down with downcut tooling" | Vortex | c |
| Down-cut router | "–" in the maximum ramp column (no legend) | SGS | c |
| Compression and down-cut | "it is best to ramp into the material" | AXYZ (a machine builder) | c |
| Up-cut spiral | "straight plunge/drill"; "Best At Plunge Cuts" | Vortex, Whiteside | c |
| PCD | "TIPICALLY CANNOT PLUNGE" | Onsrud | c |
| Non-centre-cutting | "The tool must be center cutting" (to plunge) | Harvey, Helical, CNCCookbook | c |
| SGS Z5 (5 flute) | "Do not plunge."; metal classes M, S, H: "Plunging not recommended" | SGS | c |
| Pointed engraver | plunge at 50 %; "ramping is preferred to maintain tip integrity" | Harvey SF_25000 (plastics and metals) | c |
| V-bit, tapered ball | no source forbids a plunge or a ramp. 24 statements print a plunge or ramp-down feed for them (Amana, IDC, Sienci, PreciseBits, ToolsToday). PreciseBits EM2E8: "plunge style tip geometry" | - | a-c |

Amana gives the up-cut and the down-cut Spektra part numbers one shared
"Ramp Down" row. Both Amana and Vortex are true only if "Ramp Down" is a ramp
feed, not a straight plunge (wood notes §5.4).

### 1.7 Helix start clearance (T7)

**Not published.** No statement and none of the 43 stored texts prints a start
height, a feed height or a clearance above the material for a ramp or a
helix. The repo value is the operator's ruling (0.5 mm).

### 1.8 What the data shows

1. **The plunge fraction is per family. Flat, ball and 60 deg V-bit agree
   across two or more vendors.** Flat end mill 0.50 (Sienci 60 rows, IDC
   up-cut, Amana 2F rule, Carbide 3D S3 median 0.49). Ball nose 0.50 (Sienci, Amana rule). V-bit
   0.33 (Sienci; IDC 60 deg 0.333). Tapered ball 0.50 (Sienci; IDC 0.417).
   The spread: IDC down-cut 0.30-0.43, IDC ball 0.25-0.43, Nomad 0.24-0.44.
2. **The repo plunge is well below the printed fraction on every family but
   the V-bit.** Pocket cells: 0.19-0.85 of the printed median. V-bit: 2.1-3.0
   times the printed 0.33, because the plunge scales on the 12.7 mm cone
   diameter and then clamps to the feed.
3. **The G6 drill plunge and the milling plunge disagree on the same tool**
   by 1.8-4.8x (6 mm hardwood: 1778 against 702).
4. **The approved ramp feed is the cutting feed at every repo angle.** On the
   56 flat end mill cells the second term is 2.0-42.7 x F. The form binds only
   above 26.6 deg (Z = 2). The plunge/tan fallback binds on 13 cells (E3 at
   10 deg, 3.175 mm ball, bull and tapered; 6 mm ball and bull).
5. **Helix radius has a geometric limit.** Flat r <= 0.5 D, bull r <= D/2 -
   rc (0.35 D in FM1). IMCO, Sandvik and Fusion state it in their frames; the
   geometry is the second witness. The dressup 2.0 mm default breaks it on the
   3.175 mm flat and bull tools (a core of 0.82 and 1.78 mm stands).
6. **Every ball, tapered ball and V-bit helix leaves a centre pip.** 0.32-1.2
   mm on the ball tools and 3.3-8.8 mm on the V-bit at the repo defaults
   (the dressup 2.0 mm helix leaves a core on the 3.175 mm ball tools).
7. **Ramp angle, pitch and clearance have no wood or router source.** Every
   number is metal (grade c) or a CAM user post.
8. **The wood no-plunge rules are per cut direction, not per tool kind.**
   Down cut and compression should ramp. No source forbids a plunge for a
   V-bit or a tapered tool.

### 1.9 What the data does NOT show

- No ramp angle, helix angle, helix pitch or helix diameter from a wood or
  router tool vendor.
- No definition of Amana "Ramp Down" (plunge, or feed along a ramp).
- No ramp feed as a function of angle for wood. The only such rule (SGS) is
  for metal, and it is not an axial-chip rule.
- No bull-nose plunge fraction with a feed. The only bull figures are the
  Amana 1/Z rule and PreciseBits absolute plunges with no RPM.
- No fraction at 3.175-6 mm from two vendors for the tapered ball: IDC prints
  one 1/4 in tool; Sienci prints tips of 0.5 and 1.6 mm.
- No stored source for the ball-tip cap (150 per mm), and no start clearance.
- No measurement of chip packing or burning in a helix bore in wood.

## 2. Sources

Hash: the first 16 hex digits of `raw_sha256` (the PDF or the raw HTML). The
stored text files are in `fetch/G10/sources/`. "Rehash" is the verifier's
fresh download.

| Source id | Vendor | What it prints (entry only) | Grade | Verified | sha256 |
|---|---|---|---|---|---|
| `wood_amana_compression_v8` | Amana | Ramp Down column, rule F / Z | a, b | match; 5 confirmed | `9a80df42506f8cd7` |
| `wood_amana_spektra_plunge_v24` | Amana | Ramp Down column, rule F / Z | a, b | match; 4 confirmed | `5b6fef854b2cf6b4` |
| `wood_amana_ball_nose_v7`, `wood_amana_corner_radius_plunge` (ToolsToday host) | Amana | rule F / Z only | b | match; 1 + 1 confirmed | `e851a270a9958a9a`, `a8fb36819675f9f9` |
| `wood_amana_insert_vgroove_v16` | Amana | Ramp Down column (1F 40-110 deg = F/2 against the rule) | a, b | match; 4 confirmed | `d3da2d5fd7b60aba` |
| `wood_amana_spektra_engraving_v4` | Amana | rule F / Z | b | match; 1 confirmed | `30e58f4d88e163f9` |
| `wood_idc_feeds_speeds` | IDC Woodcraft | plunge column per family | a | match; 14 confirmed | `87405efbd74696b0` |
| `wood_toolstoday_calc_video` | ToolsToday | "ramp-down or plunge rate" = F/2 | b | changed (text same); 3 confirmed | `d67334f1d5ba8b12` |
| `wood_toolstoday_understanding_feeds_speeds` | ToolsToday | "about half" | c | match; 1 confirmed | `ecbffa90b4aec6f3` |
| `wood_vortex_catalog` | Vortex | down cut must not plunge | c | match; 3 confirmed, 1 grade_wrong | `3553d2f3a1c0409b` |
| `wood_onsrud_routing_guide` | Onsrud | ramped plunging; PCD cannot plunge | c | match; 3 confirmed | `79dd5f6ef135ab94` |
| `wood_onsrud_pct19` | Onsrud | composite plunge 40 / feed 80 IPM | c | match; 2 confirmed | `83dbf74242ca1f57` |
| `wood_onsrud_plastics_faq2` | Onsrud | helical ramp for holes (plastics) | c | changed (text same); 2 confirmed | `82ff87594367a900` |
| `wood_whiteside_cnc_brochure` | Whiteside | up cut "Best At Plunge Cuts" | c | match; 1 confirmed | `00c71165ee906b3a` |
| `wood_axyz_compression_bit_tip` | AXYZ | compression: ramp | c | changed (text same); 1 confirmed | `a650eedc85c91087` |
| `metal_harvey_ramping_success` | Harvey | ramp 3-10 / 1-3 deg | c | changed (text same); 3 confirmed | `459eeab5cc8970e6` |
| `metal_harvey_tool_entry` | Harvey | helix ">110-120%"; centre cutting; 50 % | c | changed (text same); 7 confirmed | `85cc3e6e561fd7b5` |
| `metal_helical_guidebook_2016` | Helical | same rules | c | match; 5 confirmed | `6d6ca79db0b49136` |
| `metal_harvey_sf_18700` | Harvey | chamfer plunge 40-50 % | c | match; 1 wrong (corrected) | `496173bab219ba5a` |
| `metal_harvey_sf_25000` | Harvey | engraver plunge 50 %, ramp preferred | c | match; 2 wrong (corrected) | `65da065ac2c7e62a` |
| `metal_sgs_catalog_2021` | KYOCERA SGS | ramp angle per series; ramp and plunge feed | c | match; 18 confirmed | `27a309c563b43c35` |
| `metal_imco_helical_ramp` | IMCO | bore 2D - 2rc; helix angle per series | c | match; 8 confirmed | `d51a2c6ce8b8407a` |
| `metal_garr_technical` | Garr | plunge 50 % | c | match; 2 confirmed | `967eb211c24c5288` |
| `metal_sandvik_ramping` | Sandvik | hole 2 x D3; pitch <= ap; linear 75 % | c | changed (text same); 6 confirmed | `ad7b072ce0edf83d` |
| `metal_kennametal_ramping_blog` | Kennametal | prose | c | changed (text same); 1 confirmed | `0fc049680184914f` |
| `precisebits_fret_plane` | PreciseBits | bull plunge by Janka | a | match; 3 confirmed | `7ea1ad3fd8cd2cd7` |
| `precisebits_vtip_2500` | PreciseBits | V-tip plunge 40-250 IPM | b | match; 1 confirmed | `0e57846f903d20c1` |
| `precisebits_vtip_scoreengrave` | PreciseBits | micro V plunge 5-25 IPM | b, c | match; 2 confirmed | `35a04123edc1843f` |
| `precisebits_tapered_ball_2f`, `precisebits_point_styles` | PreciseBits | tapered plunge "depends on speed (RPM)"; ball is a plunge point | c | match; 1 + 2 confirmed | `cd0b38a9c270bf21`, `7d8ee8e5e68034dc` |
| `carbide3d_s3_feeds_250` | Carbide 3D | wood feed / plunge, 1/4 in | b | unreachable (G9 bytes equal); 5 confirmed | `288543263d3fec1b` |
| `carbide3d_nomad883_feeds_125` | Carbide 3D | wood feed / plunge, 1/8 in | b | unreachable (G9 file equal); 5 confirmed | `e4cd0a15950886ef` |
| `carbide3d_community_ramping_cc_pro` (staff), `carbide3d_community_ramp_entry_angle` (user) | Carbide 3D forum | CC Pro ramp max(F/3, plunge); CC default 20 deg | c | changed (quotes verbatim); 2 + 2 confirmed | `8401947341d1d2ac`, `f8a99500b27f7d85` |
| `sienci_feeds_speeds_metric` | Sienci | 106 wood rows, feed and plunge | a | match; 106 confirmed | `e7c4519025ce1a28` |
| `sienci_lm_feeds_and_speeds` | Sienci | 100-300 mm/min prose | c | match; 1 confirmed | `8459a9484662552e` |
| `vectric_v12_profile_toolpath`, `vectric_tool_database_v11` | Vectric | ramp at the plunge rate | c | match; 2 + 1 confirmed | `b42d9d95ace0f28b`, `dc4881ed6115fdae` |
| `fusion_adaptive_roughing_reference` | Autodesk | helix diameter <= D; example 0.8 D | c | match; 3 confirmed, 1 grade_wrong | `780839ed3ad78a1c` |
| `cnccookbook_helical_ramp_angle` | CNCCookbook | slot feed / Z; 1.5-2.5 deg; OSG 10-20 (second hand) | c | match; 5 confirmed | `67b162a8c484c09d` |
| `shopbot_user_guide_2015` | ShopBot | Z about 0.5 in/s | c | unreachable (stored PDF checked); 1 confirmed | `676bdd3a04a11f1b` |

Two stored sources give 0 statements: `metal_harvey_sf_809500` (a Harvey
wood chart with no entry value, evidence of absence; match, `b5be3211a863a407`)
and `metal_harvey_helical_interp_tv` (a video page; changed, `a6eed1f6196bffeb`).

Dead ends (17 records, detail in `fetch/G10/FETCH_NOTES.md` §4): Amana
product and technical pages (403); Freud 2017 and Vortex chip-load chart (no
entry text); Kennametal (6 URLs); Helical S&F index; Lakeshore; Carbide
Create manual, guides and library; ShopBot charts 2016; Onefinity; wood
literature (Crossref no study, Semantic Scholar rate-limited).

## 3. For the rulings

### 3.1 Forms per parameter and sub-class

| Parameter x tool kind | Forms it could take | Range in the data | Second witness | Recommendation |
|---|---|---|---|---|
| Helix radius, flat and bull | cap r <= D/2 - rc (no core); a minimum r >= 0.05-0.10 D (Harvey, bore reading) | max 0.5 D (flat), 0.35 D (FM1 bull) | geometry; IMCO, Sandvik, Fusion in three frames | **generic** cap (geometry). The default stays a named rule (0.3 D) |
| Helix radius, ball, tapered ball, V-bit | pip height h(r) on the card; no cap can make h = 0 | h 0.32-1.2 mm (ball), 3.3-8.8 mm (V-bit) at the defaults | geometry | **generic** geometry line on the card ("leaves a centre pip of h mm"). Do not refuse: the style is the operator's |
| Helix radius, dressup 2.0 mm absolute | keep; or the same 0.3 x D rule as Adaptive3d | 0.16-0.63 x D across the matrix | geometry shows the core at 3.175 mm | **keep a named rule on the card**, as a fraction of D (question 6) |
| Helix pitch | a pitch; or a helix angle (pitch = 2 pi r tan theta) | repo 1.8-18.5 deg; metal context 0.5-10 deg | none for wood | **keep the repo default as a named rule on the card**; the card shows the derived angle |
| Ramp angle, every kind | an angle per operation (today); per family | repo 3, 10, 12 deg; metal context 1-10 deg (soft 3-10) | none for wood | **keep the repo default as a named rule on the card** |
| Plunge, flat end mill | fraction of side feed; axial chip 1/Z (G6) | 0.24-0.53, median 0.50 | Sienci 60 rows + Amana rule + IDC up cut + Carbide 3D | **per family**: plunge = side feed / Z (Z = 2: 0.50) from the tool's own row |
| Plunge, ball nose | fraction of side feed | 0.25-0.50, median 0.50 | Sienci + Amana rule; IDC 0.25-0.43 | **per family** 0.50; the tip cap stays a named rule |
| Plunge, tapered ball | fraction of side feed | 0.33-0.50, median 0.50 | Sienci; IDC 0.417 (one tool) | **per family** 0.50, one witness per size; the tip cap stays |
| Plunge, bull nose | Amana 1/Z rule; PreciseBits absolute | 0.50 (rule); 320-600 per mm | none with a feed | **keep the repo default as a named rule on the card**; cite the Amana rule as the nearest |
| Plunge, V-bit | fraction of side feed | 0.33 (Sienci, IDC 60 deg); IDC 0.33-0.57; Amana Ramp Down 0.5-1.0 | Sienci + IDC at 60 deg | **per family** 0.33 for the 60 deg V-bit |
| Ramp feed, flat end mill (56 cells) | approved form min(F, F / (Z tan theta)) | = F at every repo angle | Amana (both readings of Ramp Down); SGS disagrees (metal) | as approved; the card states that it equals F and why |
| Ramp feed, other kinds (172 cells) | literal plunge; min(F, plunge / tan theta) | plunge/tan = F on 159 cells, below F on 13 | none | question 2 |
| Entry clearance | 0.5 mm (ruled) | not published | none | **refuse** (card: "no source; operator rule 0.5 mm") |
| Entry style | operator setting (ruled) | - | - | Suggest chooses nothing; see 3.3 |

"Refuse" here means the card says "no source" and the engine still uses the
named rule. Every rough needs an entry, so an entry parameter cannot refuse
the way a chip load does. A card line reads, for example: "Ramp angle 3.0
deg: repo rule, no wood or router source (metal context 3-10 deg, Harvey,
grade c)." A metal range is context on the card, never the source of a wood
number.

### 3.2 Engine rules the trend puts in question

- The dressup helix (2.0 mm absolute) leaves a core on the 3.175 mm tools;
  the Adaptive3d helix (0.3 D, pitch 2) is 18.5 deg on them.
- The V-bit plunge scales on the widest cone diameter, then clamps to the
  feed (18 cells ship plunge = feed).
- `FeedsResult::ramp_feed_mm_min` (clamp(0.5 F, plunge, 1.5 plunge)) has no
  reader and no source. The approved field replaces it.
- The plunge provenance names the chip load's LUT row (INVENTORY_G10 3.3),
  but only the 16 G6 plunges are printed. The ball-tip cap cites an unstored
  FSWizard / GWizard range.

### 3.3 Where the code still chooses the entry style (a finding)

The entry style is the operator's setting (ruled). The code still sets or
changes it in these places (INVENTORY_G10 §3.2):

1. `pick_adaptive3d_entry_style` (`feeds/suggest/adaptive_entry.rs:62`):
   Suggest rewrites Plunge to Ramp or Helix. It fires on all 30 ok
   Adaptive3d cells.
2. `DressupConfig::for_role` Roughing -> Ramp (`compute/config.rs:1099`): a
   default on E1 (168 ok cells).
3. `PreferHelix` (`compute/config.rs:1193`): turns an operator Ramp into
   Helix on 2D Adaptive, on every dressup write and on project load (30 ok).
4. The CLI `entry` mapping writes its own angle and helix set.

This record recommends no rule that has Suggest choose a style. Items 1 and 3
conflict with the ruling; item 2 is a default, not a choice.

### 3.4 Questions the operator must rule

Cell counts are ok cells from `g10_inventory_cells.csv` (T8).

1. **Ramp feed, flat end mill.** The approved form ships F at 3, 4.55 and
   10 deg (term / F 2.0-42.7). Accept "ramp feed = F", with a card line that
   states the 1/Z derivation? Recommend: accept. Moves EndMill E1-E3, 56.
2. **Ramp feed fallback, other kinds.** (a) ramp feed = plunge (the approval
   text), or (b) min(F, plunge / tan theta), which holds the vertical rate at
   the plunge: F on 159 cells, less on 13. Recommend: (b), card "no source;
   vertical rate = plunge (repo rule)". Moves Bull 56, Ball 40, Tapered 56,
   VBit 20 (172).
3. **Amana "Ramp Down".** Read it as the vertical (Z) rate: the plunge and
   the vertical limit of a ramp, as B5 reads it. Recommend: accept. It is the
   premise of questions 1 and 4.
4. **Milling plunge, flat end mill.** Plunge = side feed / Z from the tool's
   own row (the G6 form, 3.175-6.0 mm, 2-3 flutes), not the material base?
   Witnesses: Amana, Sienci (60 rows), IDC up cut, Carbide 3D. Recommend: per
   family; the card names the down-cut spread (IDC 0.30-0.43). Moves EndMill
   E1-E6, 120 (Pocket plunge x2.0-5.4, derived: F / Z over the CSV plunge).
5. **Plunge fraction, ball / tapered ball / V-bit.** 0.50 / 0.50 / 0.33 of
   the side feed; the tip cap stays a named rule. Recommend: per family for
   ball and the 60 deg V-bit (two vendors each); tapered ball per family with
   one witness per size, or the named rule. Moves Ball 98, Tapered 152, VBit
   48 (the V-bit plunge falls 2.1-3.0x).
6. **Helix radius.** A generic no-core cap (flat r <= D/2, bull r <= D/2 -
   rc), and the dressup default as a fraction of D? Recommend: cap generic
   (geometry); default a named rule (0.3 x D, as Adaptive3d). Moves E2 30 and
   every operator helix on the 3.175 mm tools.
7. **Helix pip.** Show the derived pip height on the card for ball, tapered
   ball and V-bit, and do not refuse? Recommend: accept. Moves the card on E2
   Ball 6, Tapered 8, and on any operator helix on those tools.
8. **Helix pitch.** Keep 1.0 / 2.0 mm as a named rule and show the derived
   angle? Recommend: accept. Optional: state the rule as an angle, so that it
   does not reach 18.5 deg on small tools. Moves E2 30, E3 30 (helix).
9. **Ramp angle.** Keep 3 deg (dressup) and 10 deg (Adaptive3d) as named
   rules, metal range as context? Recommend: accept, and make the CLI
   Adaptive3d 3 deg and the GUI 10 deg one rule. Card on E1 168, E3 30.
10. **Entry clearance.** Card "no source; operator rule 0.5 mm". Recommend:
    accept. Card on E1-E3, 228.
11. **Entry style chooser.** Remove the Suggest rewrite and `PreferHelix`
    (3.3 items 1 and 3), and keep the warning `check_plunge_entry_stability`?
    Recommend: accept. Moves E3 30 (back to Plunge) and E2 30 (an operator
    Ramp stays).
12. **Down-cut and compression tools.** The engine has no cut-direction or
    centre-cutting attribute. Record it as future work, with a card warning on
    Plunge? Recommend: accept as future work. Moves 0 FM1 cells.

## 4. Errors found in the fetch and the inventory

- Fetch: the five open verifier findings in `fetch/G10/FETCH_NOTES.md` §3
  (the SGS `flutes` null on two rows; the Nomad "G9 record" claim; the Fusion
  2 deg example against the hobby dead-end table; the metal text headers dated
  2026-09-24; the Sienci web table against its PDF). None changes a §1 table.
- Inventory: no error found in the rows this record reads. The derived
  numbers in INVENTORY_G10 §2.3 and §3.5 recompute (for example 12 900-25 900
  mm/min for the 6 mm hardwood helix term).
