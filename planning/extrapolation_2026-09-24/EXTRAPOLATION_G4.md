# G4: Band (one value printed, no minimum)

Status: Phase 2 (trend) done 2026-09-24; Phase 3 (fit, witness) not started; nothing lands before the Phase 4 rulings.

Inputs: the LUT at 4098e937 (`crates/rs_cam_core/data/vendor_lut/observations/*.json`),
`fetch/G4/` (Phase 1 fetch and its verifier verdicts), `inventory_cells.csv`
(Phase 0). Script: `scripts/trend_g4.py` (read-only; run
`python3 scripts/trend_g4.py`). Every number in §1 and §3 that is not a
quoted chart value is **derived** by that script. No vendor prints it.

## 0. The gap

The Phase 0 inventory puts **83 shipping cells** in G4. They sit on
**16 LUT rows**. Each row prints one chipload value and no minimum.

| Anchor rows | Encoding | Cells | Source |
|---|---|---|---|
| 14 Amana Spektra 2-flute rows, 3.175 mm and 6.0 mm | `chipload_min_mm_tooth` absent | 75 | `amana_spektra_spiral_plunge_v24` (4 MDF rows grade a exact; 10 Wood/Plywood rows grade b derived, applied to softwood, hardwood and hardwood plywood) |
| 2 Amana AMS-159 60° 2-flute V-groove rows, 12.7 mm | `chipload_min == chipload_max` = 0.0762 mm (0.003 in) | 8 | `amana_ams159_vgroove_v2` (grade a exact) |

Counts by axis (from `trend_g4.py` §6):

| Axis | Counts |
|---|---|
| Tool | EndMill 44, BullNose 31, VBit 8. The BullNose cells resolve to the flat-end Spektra rows. |
| Operation | Adaptive 14, Adaptive3d 11, Face 11, Pocket 11, Rest 11, Zigzag 11, DropCutter 6, Chamfer 2, Inlay 2, Trace 2, VCarve 2 |
| Material | hardwood 28, mdf 26, plywood_hardwood 21, softwood 8 |

Related, not in the 83: 7 hardwood cells (4 EndMill, 3 BullNose) ship on the
14 repo-authored Spektra "band" rows (grade c, D1 in
`fetch/G4/lut_discrepancies.md`). Those rows fill the same gap with an
unsourced band. Phase 3 must treat them as G4 cells, not as printed bands.

### The current engine rule

The engine has two encodings of "one value printed", and they behave
differently (D2). The code was read at 4098e937.

| Consumer | Function (file) | min absent (75 cells) | min == max (8 cells) |
|---|---|---|---|
| Bound policy | `derate_chipload_bounds` with `ChiploadBoundPolicy::RequireBoth` / `AllowHalfBand` (`feeds/geometry.rs` L255-289) | `RequireBoth` returns `None`. `AllowHalfBand` returns a half band (max only). | Both policies accept the row (`min <= max`). |
| Burn / breakage gate | `tool_load/chipload.rs` L585 (`AllowHalfBand`) | High side hard. The low side is not modelled. | Source class `ChipBoundsSource::VendorLutPointPreset`. `low_side_is_advisory` (`tool_load/verdict.rs` L1292) is true, so a low-side trip gives a burn advisory only. High side hard. |
| Envelope resolver | `chipload_envelope_for_toolpath` (`tool_load/mod.rs` L302, `RequireBoth` at L364) | `None`. The printed value is dropped. | A zero-width range `v..v`. |
| Simulation modulator | BANDLESS arm (`dressup/feed_modulation.rs` L767) | Commanded feed, clamped to the machine ceiling. No chipload target or cap. | `ChiploadBand::new(v, v)`: target and floor are one feed. |
| Strategy advisor | `optimized_candidate` (`session/compute.rs` L1496) | Returns `None`. It does not optimise. | Optimises inside a zero-width band. |
| Suggest | `feeds/mod.rs` L1627-1636; `effective_rubbing_floor` (L1229) | One line. The rubbing floor falls back to `RUBBING_FLOOR_MM_TOOTH` = 0.025 mm (L1142). | Bounds `v..v`. |

No consumer refuses a G4 cell. Every G4 cell ships. The gap is the missing
low bound, not a refusal.

## 1. The trend

### 1.1 What the trend counts

The script keeps a wood row as a **printed band** when all of these are true:

- min and max are present, and min < max;
- the evidence grade is a or b;
- the notes do not say the band is not printed (regex
  `not printed|no printed row|repo-authored|derived \(reduced\)|extrapolated`).

The script then removes duplicates. The LUT copies one printed chart cell
into several operation rows. The key is (source, family, subfamily,
diameter, flutes, material, min, max). Result: **176 rows, 143 printed
cells**. `verified_rows.json` adds 0 rows (§2).

Band rows excluded (all grade c; none is printed on its chart):

```
    7  ball_nose          amana_ball_nose_v7                       grade c   ("Derived (reduced)" 3D-finish bands)
    3  bull_nose          onsrud_cutting_data_recommendations      grade c   (seeded from a Shapeoko reference)
   14  flat_end           amana_spektra_spiral_plunge_v24          grade c   (D1 repo-authored bands)
    8  tapered_ball_nose  amana_zrn_3d_profiling                   grade c   ("No printed row ... extrapolated")
```

So **bull nose has no printed band**. The "bull 0.54" in `lut_axes.txt` is
the repo-authored rows. The flat-end printed bands come from Onsrud (80
cells), Freud (12) and Amana ZrN v8 (4) only.

Differences from `lut_axes.txt` and `fetch/G4/band_shape_derived.txt`
(those scripts count rows, with no grade filter and no de-duplication):

| Family | lut_axes median | trend_g4 median | Why |
|---|---|---|---|
| flat_end | 0.88 (n 110) | 0.88 (n 96) | 14 D1 rows out |
| ball_nose | 0.62 (n 27) | 0.71 (n 14) | 7 reduced rows out; duplicates out |
| tapered_ball_nose | 0.60 (n 40) | 0.66 (n 8) | 8 Amana ZrN grade-c rows out; the LUT holds each printed Onsrud cell as 4 rows (2 pass roles x 2 operation families) |
| bull_nose | 0.54 (n 3) | no data | all 3 are grade c |
| chamfer_vbit | 0.43 (n 20) | 0.43 (n 17) | duplicates out |

### 1.2 min/max ratio per tool family, vendor, material category, size band

```
### per tool family
  ball_nose                                    n= 14  median 0.71  Q1-Q3 0.43-0.75  range 0.38-0.78
  chamfer_vbit                                 n= 17  median 0.43  Q1-Q3 0.43-0.47  range 0.33-0.55
  facing_bit                                   n=  8  median 0.56  Q1-Q3 0.55-0.56  range 0.54-0.57
  flat_end                                     n= 96  median 0.88  Q1-Q3 0.83-0.89  range 0.40-0.92
  tapered_ball_nose                            n=  8  median 0.66  Q1-Q3 0.60-0.71  range 0.60-0.71

### per tool family / vendor
  ball_nose / amana                            n= 14  median 0.71  Q1-Q3 0.43-0.75  range 0.38-0.78
  chamfer_vbit / amana                         n= 15  median 0.43  Q1-Q3 0.43-0.46  range 0.33-0.55
  chamfer_vbit / whiteside                     n=  2  median 0.51  Q1-Q3 0.51-0.52  range 0.50-0.53
  facing_bit / amana                           n=  6  median 0.56  Q1-Q3 0.54-0.57  range 0.54-0.57
  facing_bit / whiteside                       n=  2  median 0.56  Q1-Q3 0.55-0.56  range 0.55-0.56
  flat_end / amana                             n=  4  median 0.68  Q1-Q3 0.60-0.76  range 0.60-0.78
  flat_end / freud                             n= 12  median 0.80  Q1-Q3 0.67-0.86  range 0.40-0.88
  flat_end / onsrud                            n= 80  median 0.89  Q1-Q3 0.86-0.89  range 0.67-0.92
  tapered_ball_nose / onsrud                   n=  8  median 0.66  Q1-Q3 0.60-0.71  range 0.60-0.71

### per tool family / material category  (solid = softwood+hardwood; sheet = MDF/HDF/particleboard)
  ball_nose / sheet                            n=  4  median 0.54  Q1-Q3 0.38-0.72  range 0.38-0.75
  ball_nose / solid                            n= 10  median 0.71  Q1-Q3 0.60-0.76  range 0.38-0.78
  chamfer_vbit / plywood                       n=  1  median 0.50
  chamfer_vbit / sheet                         n=  2  median 0.50  Q1-Q3 0.48-0.51  range 0.46-0.53
  chamfer_vbit / solid                         n= 14  median 0.43  Q1-Q3 0.43-0.46  range 0.33-0.55
  facing_bit / sheet                           n=  4  median 0.56  Q1-Q3 0.55-0.56  range 0.54-0.57
  facing_bit / solid                           n=  3  median 0.56  Q1-Q3 0.55-0.56  range 0.54-0.56
  flat_end / plywood                           n= 35  median 0.90  Q1-Q3 0.88-0.91  range 0.67-0.92
  flat_end / sheet                             n= 24  median 0.87  Q1-Q3 0.76-0.89  range 0.57-0.89
  flat_end / solid                             n= 37  median 0.86  Q1-Q3 0.78-0.88  range 0.40-0.89
  tapered_ball_nose / (each category)          n=2-4  median 0.66  range 0.60-0.71

### per tool family / size band (mm; bins are a script choice)
  ball_nose / <3                               n=  6  median 0.38  Q1-Q3 0.38-0.55  range 0.38-0.77
  ball_nose / 3-6                              n=  3  median 0.71  Q1-Q3 0.66-0.71  range 0.60-0.71
  ball_nose / 6.35-9.5                         n=  4  median 0.75  Q1-Q3 0.74-0.76  range 0.71-0.78
  ball_nose / >=12.7                           n=  1  median 0.78
  chamfer_vbit / 3-6                           n=  4  median 0.47  Q1-Q3 0.47-0.48  range 0.46-0.50
  chamfer_vbit / 6.35-9.5                      n=  8  median 0.43  Q1-Q3 0.40-0.43  range 0.33-0.43
  chamfer_vbit / >=12.7                        n=  3  median 0.53  Q1-Q3 0.51-0.54  range 0.50-0.55
  flat_end / 3-6                               n= 15  median 0.83  Q1-Q3 0.63-0.86  range 0.40-0.88
  flat_end / 6.35-9.5                          n= 45  median 0.88  Q1-Q3 0.86-0.89  range 0.67-0.92
  flat_end / >=12.7                            n= 36  median 0.89  Q1-Q3 0.86-0.90  range 0.71-0.92
  tapered_ball_nose / 3-6                      n=  4  median 0.60
  tapered_ball_nose / 6.35-9.5                 n=  4  median 0.71

### flat_end per vendor / size band (the G4 flat cells are 3.175 and 6.0 mm: the 3-6 bin)
  amana / 3-6                                  n=  3  median 0.60  Q1-Q3 0.60-0.69  range 0.60-0.78
  freud / 3-6                                  n=  3  median 0.57  Q1-Q3 0.49-0.62  range 0.40-0.67
  onsrud / 3-6                                 n=  9  median 0.86  Q1-Q3 0.83-0.86  range 0.83-0.88
  freud / 6.35-9.5                             n=  5  median 0.76  Q1-Q3 0.73-0.83  range 0.67-0.88
  onsrud / 6.35-9.5                            n= 39  median 0.89  Q1-Q3 0.87-0.89  range 0.67-0.92
  freud / >=12.7                               n=  4  median 0.86  Q1-Q3 0.86-0.86  range 0.85-0.87
  onsrud / >=12.7                              n= 32  median 0.89  Q1-Q3 0.88-0.90  range 0.71-0.92
```

### 1.3 Absolute band width (inches, the charts' unit)

```
### per tool family
  ball_nose                                    n= 14  median 0.0020  Q1-Q3 0.0013-0.0020  range 0.0002-0.0020
  chamfer_vbit                                 n= 17  median 0.0040  Q1-Q3 0.0016-0.0040  range 0.0012-0.0040
  facing_bit                                   n=  8  median 0.0026  Q1-Q3 0.0026-0.0028  range 0.0026-0.0031
  flat_end                                     n= 96  median 0.0020  Q1-Q3 0.0020-0.0020  range 0.0020-0.0040
  tapered_ball_nose                            n=  8  median 0.0020  Q1-Q3 0.0020-0.0020  range 0.0020-0.0020

### share of cells whose width is 0.002 in (+/- 0.00005 in)
  ball_nose          amana        8 of  14
  chamfer_vbit       amana        1 of  15
  flat_end           amana        4 of   4
  flat_end           freud        3 of  12
  flat_end           onsrud      79 of  80
  tapered_ball_nose  onsrud       8 of   8

### ratio and width against diameter, flat_end (least-squares slope)
  amana      n=  4  d 3.17-6.35 mm   ratio slope +0.0537/mm  width slope  0.000000 in/mm
  freud      n= 12  d 3.17-12.70 mm  ratio slope +0.0303/mm  width slope +0.000046 in/mm
  onsrud     n= 80  d 3.17-19.05 mm  ratio slope +0.0022/mm  width slope +0.000004 in/mm
```

### 1.4 Where the one printed value sits against other charts

For each G4 anchor, the script lists the printed bands of other sources for
the same tool family, the same material family and a diameter within 6 %.
Tally over the unique (family, material, diameter) anchors:

| Family | v below the other band's min | v = min | v inside | v = max | v above max |
|---|---|---|---|---|---|
| flat_end (8 anchors, 21 comparisons) | 13 | 3 | 3 | 2 | 0 |
| chamfer_vbit (2 anchors, 2 comparisons) | 0 | 0 | 2 | 0 | 0 |

Examples (from `trend_g4.py` §8): Spektra 6 mm hardwood .0050 in equals the
min of the Onsrud 60-200 downcut band (.0050-.0070) and is below the Freud
band (.0080-.0110). Spektra 1/8 in MDF .0050 in equals the max of the Amana
ZrN v8 band (.0030-.0050) and is inside the Freud band (.0040-.0070).
The Onsrud compression bands start 2.0-3.0x above the Spektra value (v/min 0.33-0.50).

### 1.5 The same-sheet observation (AMS-159)

The AMS-159 chart (stored text `fetch/G5/sources/amana_ams159_vgroove_v2.txt`)
prints, for every wood row:

- 18°, 30°, 45°, 1 flute: `50" - 130"` IPM, `0.003" - 0.007"` per tooth;
- 60°, 90°, 2 flute: `90"` IPM, `0.003"` per tooth.

The single value equals the low edge of the band on the same sheet, per
tooth. Derived: per revolution the 2-flute value is 0.006 in/rev, inside the
1-flute band of 0.003-0.007 in/rev. Derived: 90 IPM / (18,000 rpm x 2) =
0.0025 in/tooth, not the printed 0.003 in. This is one chart and one tool
line. It does not transfer to Spektra.

### 1.6 What the data shows

1. **One ratio per family is not supported for flat end mills at the G4
   sizes.** At 3-6 mm the three vendors' medians are 0.57 (Freud), 0.60
   (Amana ZrN) and 0.86 (Onsrud); the range is 0.40-0.88. The family
   median 0.88 is Onsrud's number: Onsrud gives 80 of 96 cells.
2. **The width is more stable than the ratio.** 86 of 96 flat-end cells,
   8 of 8 tapered cells and 8 of 14 ball cells print a width of exactly
   0.002 in. The ratio then rises with diameter, because the width is fixed
   and the value grows (Freud +0.030/mm; Amana +0.054/mm on 4 cells).
   Onsrud's ratio is near flat (+0.002/mm) because its values are large
   against 0.002 in.
3. **V-bits print wider bands.** The Amana V-bit charts print 0.004 in
   (ratio 0.33-0.43). The one-value V-bit cells come from a sheet whose
   banded columns are 0.003-0.007 in.
4. **The Spektra value is low against other charts.** In 16 of 21
   comparisons it is at or below another chart's minimum. It is never above
   another chart's maximum.
5. **The vendor texts read one value as a start point** (§2): Amana "only
   recommendations"; Freud (grade a) "recommended starting points" and
   "start your tests with the lower feed rates"; ToolsToday (grade c) "Amana's
   recommended starting calculations". No text calls a printed value a
   maximum.

Other families: facing bits (0.54-0.57, n 8) and tapered balls (0.60 and 0.71 at two sizes, a constant 0.002 in width) are tight, but each has one vendor and the spread follows the width. V-bits (0.33-0.55) are not tight.

### 1.7 What the data does NOT show

- It does not show which edge of a band a single value is. Only AMS-159
  prints a band and a single value side by side. Items 4 and 5 above point
  to "start point" or "low edge", but they are indirect for Spektra.
- It does not show that 0.002 in is physical. The charts print on a
  0.001 in grid. A two-tick band can be a print convention.
- It does not give the burn side a number. Freud and Onsrud name the
  mechanism (small chip: heat, dulling) and print no value.
- The cross-chart comparison in 1.4 mixes tool lines. A compression spiral
  and a Spektra plunge bit are different tools.
- It says nothing about bull nose bands (no printed band in the LUT).
- It gives no size law. Phase 3 fits; this section states slopes only.

## 2. Sources

Verifier verdicts: `candidate_rows.json` holds 0 rows, so every source has
0 candidate verdicts. The verifiers checked existing LUT rows instead; the
counts below are those LUT-row verdicts.

| id | Vendor | What it prints (for G4) | Grade | URL reachable | sha256 (stored text unless noted) | Verifier verdicts |
|---|---|---|---|---|---|---|
| `freud_router_bit_feed_and_speed_for_cnc_20170822` | Freud | Bands for flat end mills. "start your tests with the lower feed rates"; "recommended starting points". Both edges named, no number. No width rule. | a | yes | `ff1a29ea5a669e24c6ed18e2bd21e10321cee024c65a6b23a1197cbbfdc71976` (PDF) | LUT rows: 13 confirmed, 1 wrong (`freud-solid-carbide-quarter-hard-plastic`: Solid Surface/Hard Plastic value under an acrylic label; non-wood, handed to the LUT owner) |
| `amana_spektra_spiral_plunge_v41` | Amana | One value per cell. "these values are only recommendations". Depth derate. Halves the 1.5 mm and 3 mm rows against v24. | a | yes | `7b61543a12cb3dc9ed0716733e7e5c2822b6ebe4149a721e060e8a07cb1d7eb3` (PDF) | 0 rows. Confirmed: one value per cell, no range text. The date 2024-06-15 is PDF metadata, not printed. |
| `amana_compression_spirals_v8` | Amana | One value per cell. Same notes. | a | yes | `9a80df42506f8cd72540c795ec9272c2abb21d76e4ab0febba715c6678f59a0b` (PDF) | LUT rows: 6 confirmed (5 wood + 1 plastic) |
| `amana_spektra_spiral_plunge_v24` | Amana | The LUT's source for the 75 flat cells. One value per cell. No range, maximum or start note. | a (verification) | yes | `5b6fef854b2cf6b422e2eb5bbc86e29b4dcab5b19195b7ff228b70f99cf86d3a` (PDF, = manifest) | 100 LUT rows by script: 86 equal the printed value; 14 are D1 (grade c, repo-authored) |
| `onsrud_feedspeeds_chipload_page_wayback_20180620` | Onsrud | Both edges, qualitative: "If the chip is too small ... prematurely dulling. Too high of a chipload will cause an unsatisfactory edge finish, or part movement." | b | yes (Wayback; live 404) | `70cc4d5b4169251b8f6dc75b834e22c4c7634c510bcda86dc2695cf5f0827440` | 0 rows; quotes confirmed |
| `onsrud_cutting_data_recommendations_page_2026-09-24` | Onsrud | "recommended starting speeds and chip-load guidance". No band-use rule. | b | yes | `155cfad83bb81b4d84ac877034216aa655cbd6f770e7dbeb2f97eb2bf31eddce` | 0 rows; quote confirmed. The verifier did not check the image-only H Wood sheet. |
| `amana_router_bit_technical_information_wayback_20240528` | Amana | Hand-router text: "There is no set feed speed". No numbers. | b | yes (Wayback; live 403) | `a1df6bb86bc6f15846f51125803d006a6b72d4c3f75398bb398a666a3b44c52e` | 0 rows; quotes confirmed |
| `toolstoday_how_to_calculate_feeds_and_speeds` | ToolsToday (Amana retailer) | "these feed and speed charts are Amana's recommended starting calculations". Its example runs 0.020 in/tooth against a spoken chart value of 0.0072. | c | yes | `839ab5468ffacc2252ef31843e9099de3c1d50bb0c0803122ea1114f975816d6` | 0 rows. Verifier notes: the chart value is stated for MDF (laminate only later); 0.0072 is spoken, not printed; the 2.8x ratio is derived and "contradicts a maximum" is an inference. |
| `shopbot_feeds_and_speeds_2016` | ShopBot (restating Onsrud) | "Start with the middle of the range"; "increase ... until the quality ... starts to decrease ... Then decrease speed by 10%." | c | not verified | `7ad6d74012fe2d5f52f25c2e7c75f199be6845d5e21e52a4147892ebe779f56e` (PDF) | **not verified**; no Onsrud document prints these sentences |
| `amana_ams159_vgroove_v2` (G5 copy, cited in 1.5) | Amana | 1-flute `0.003" - 0.007"`; 2-flute 60°/90° `0.003"`. | a | yes (G5 fetch) | text `19fed534d9cbefe973f674cc4aba5cff28b835fbc2e567a8c0f5840f983d3a6e`; PDF `69628c747676f93080a011b93f3a5175d709c149adc7a13046e3eac9c6c5fa93` | G5 fetch: LUT rows match the chart |

Dead ends (from `fetch/G4/FETCH_NOTES.md` §7-8):

- `https://www.amanatool.com/router-bit-technical-information`: Cloudflare
  403. The Wayback copy is used.
- `http://www.onsrud.com/xdoc/FeedSpeeds` (cited by ShopBot): 404. The
  Wayback copy is used; it lacks both ShopBot sentences.
- `http://s3.amazonaws.com/fablab-uc/.../Feeds_and_Speeds_Chart.pdf`: an XML
  error, not a PDF.
- Spektra URL pattern v34 and v42-v60: HTML, not a PDF. v25-v33 and v35-v40
  exist; only v24, v28 and v41 were compared.
- Amana product pages: 403. So "v41 is the chart Amana links today" is not
  confirmed.
- `https://onsrud.com/images/H%20Wood%20Cutting%20Data2.pdf`: image-only;
  viewed, no text copy, no numbers taken.
- Not found anywhere: a vendor "+/- %" or band-width rule for one value; a
  vendor statement that a single value is a maximum; a vendor number for the
  burn side of a one-value chart; an Onsrud print of the ShopBot sentences.

## 3. For Phase 3

### 3.1 The question Phase 4 must rule first

The LUT encodes the single value as `chipload_max` ("encoded as
chipload_max only"). No source says it is a maximum. A derived minimum below
the value (rules A and B) assumes the value is the top of a band. A band
above the value (rule C) assumes it is the start. The two readings give
different burn floors and different breakage caps. This report does not
choose. The evidence for each reading:

- Top reading: the LUT's current encoding; the burn gate's hard high side.
  No printed text.
- Start reading: Freud (a) "recommended starting points", "start your tests
  with the lower feed rates"; Amana (a) "only recommendations"; ToolsToday
  (c) "recommended starting calculations"; AMS-159 (a) single value = the
  sibling band's low edge; §1.4 (Spektra at or below other charts' minimum
  in 16 of 21 comparisons).

A second item precedes any fit: the engine must pick **one encoding** for
"one value printed" (D2). Today the same printed fact reaches the modulator
as "no band" (min absent) or as "zero-width band" (min == max). That is an
engine item, not a G4 fit.

### 3.2 Candidate forms and the band each gives

All bands are derived. v = the printed value. The R1 check is the 0.5-2x
judgement threshold (min >= 0.5v, max <= 2v). The floor check is
min >= 0.025 mm (`RUBBING_FLOOR_MM_TOOTH`).

| Rule | Form | Reading |
|---|---|---|
| A | [v - w, v], w = the family median width (flat 0.002 in, V-bit 0.004 in) | top |
| A2 | [v - 0.002 in, v] (V-bit only) | top |
| B | [r_med x v, v], r_med = family median ratio (flat 0.88, V-bit 0.43) | top |
| B- | [r_min x v, v], r_min = lowest printed ratio (flat 0.40, V-bit 0.33) | top |
| C | [v, v + w] | start |
| C2 | [v, v + 0.002 in] (V-bit only) | start |

The 83 cells (16 rows, grouped by printed value):

| Anchor rows | Cells | v mm (in) | A | B | B- | C |
|---|---|---|---|---|---|---|
| Spektra 3.175 mm Wood/Plywood: hardwood (pocket 11, adaptive 2), plywood_hardwood (pocket 11, adaptive 2), softwood (adaptive 2) | 28 | 0.1016 (.0040) | 0.0508-0.1016 (0.50v, at the R1 edge) | 0.0889-0.1016 (0.88v) | 0.0406-0.1016 (0.40v, fails R1) | 0.1016-0.1524 (1.50v) |
| Spektra 3.175 mm MDF (pocket 11, adaptive 2) | 13 | 0.1270 (.0050) | 0.0762-0.1270 (0.60v) | 0.1111-0.1270 | 0.0508-0.1270 (fails R1) | 0.1270-0.1778 (1.40v) |
| Spektra 6.0 mm Wood/Plywood: hardwood (pocket 11, adaptive 1), plywood_hardwood (pocket 6, adaptive 2), softwood (adaptive 1) | 21 | 0.1270 (.0050) | 0.0762-0.1270 (0.60v) | 0.1111-0.1270 | 0.0508-0.1270 (fails R1) | 0.1270-0.1778 (1.40v) |
| Spektra 6.0 mm MDF (pocket 11, adaptive 2) | 13 | 0.1524 (.0060) | 0.1016-0.1524 (0.67v) | 0.1334-0.1524 | 0.0610-0.1524 (fails R1) | 0.1524-0.2032 (1.33v) |
| AMS-159 60° 12.7 mm, hardwood 3 + softwood 5 (Chamfer, Inlay, Trace, VCarve) | 8 | 0.0762 (.0030) | -0.0254: **refuse** (min <= 0); A2 0.0254-0.0762 (0.33v, fails R1) | 0.0327-0.0762 (0.43v, fails R1) | 0.0254-0.0762 (fails R1) | 0.0762-0.1778 (2.33v, fails R1); C2 0.0762-0.1270 (1.67v) |

Every derived minimum is above the 0.025 mm floor. The per-cell list is in
`fetch/G4/trend_g4.out` §6 (the script output, 2026-09-24).

Rule A along the whole Spektra one-value series (`trend_g4.py` §7): A gives
min >= 0.5v only for v >= .0040 in. For v <= .0030 in (0.79-2.38 mm sizes)
A gives 0.33v or less, zero or a negative value. **Rule A cannot serve
Spektra below 3 mm.** v41 halves the 3 mm row to .0020 in (D3); if the LUT
moves to v41, A refuses the 3 mm row too. The 3.175 mm row (.0040 in) is
unchanged in v41.

### 3.3 Second witnesses that could exist

- **The same sheet.** AMS-159 prints a band next to a single value. A
  second one-value-plus-band sheet from another vendor would make the start
  reading a two-witness claim. None is in hand.
- **Simulation.** The chip samples of a real part (wanaka, terrain) under
  each candidate band: does the modulator floor bind, and where? This tests
  the consequence, not the vendor meaning.
- **A physical rule.** Chip thickness against the cutting-edge radius
  (rubbing onset). Derived: 0.002 in = 0.0508 mm, which is 2x the repo's
  0.025 mm floor. The floor itself carries no source in this group.
- **A literature cell.** The literature matrix for a burn-side chipload in
  wood. Not searched in G4.
- **A newer Spektra chart.** v41 (G1 material) changes the anchors below
  3.175 mm. Any rule fitted on v24 values must be re-checked on v41.

### 3.4 Recommendation per sub-class

| Sub-class | Cells | Recommendation | Why |
|---|---|---|---|
| Spektra flat end, 3.175 and 6.0 mm (EndMill and BullNose) | 75 | **per-family, conditional**. Refuse a derived minimum until Phase 4 rules the reading. If the top reading wins: rule A (width 0.002 in, 0.50-0.67v) is the only width-based form, and it is inside R1 for all 75, one witness (the printed widths of Onsrud, Freud and Amana ZrN). Rule B (0.88v) is also inside R1, but the trend does not support one ratio: the vendor medians at these sizes are 0.57, 0.60 and 0.86. Rule B- fails R1. If the start reading wins: rule C (1.33-1.50v, inside R1) raises the breakage cap, which needs a second witness before it ships. | §1.6 items 1, 2, 4, 5 |
| Spektra flat end below 3 mm | 0 today (G1 cells) | **refuse** under rule A | A gives min <= 0.33v (§3.2) |
| AMS-159 60° / 90° V-groove, 12.7 mm | 8 | **per-chart with a borrowed width, or refuse**. One witness (the same sheet). Every top-reading rule fails R1 or goes negative. The V-bit-native width (0.004 in, rule C) gives the same-sheet band [.0030, .0070 in], which fails R1 (2.33v). Only C2 [.0030, .0050 in] (1.67v) is inside R1, and its 0.002 in width is borrowed from the flat-end and tapered charts. Phase 4 rules whether a borrowed width is acceptable. | §1.5; §3.2 |
| The 7 cells on D1 repo-authored bands | 7 | Treat as G4 cells. Replace the unsourced band by the ruled G4 form. | D1 |
| Bull nose bands | none printed | **refuse** a bull-nose ratio or width | no printed band in the LUT |
| Generic (all families) | — | **not supported** | the ratio differs by family (0.43-0.88) and by vendor inside a family; the width differs by family (0.002 vs 0.004 in) |

### 3.5 Biggest open gap

No document says what a single printed value is. Every G4 form depends on
that reading, and only one grade-a sheet (AMS-159, a different tool line)
shows a single value next to a band.

## 5. The landing (A2 point mode, 2026-09-24)

Ruling A2: a single printed value is held as a point; no band is derived.
Plan and decisions: `A2_PLAN.md`.

| Commit | Step |
|---|---|
| fe1fbbd3 | `PrintedChipload`, the build_result normalization (min == max -> min absent), the gate (`{min None, max v'}`, hard above, burn advisory below naming "the printed point"), `chipload_point_mm`, the rubbing floor at the point, the card text |
| e7611194 | the modulator's point arms (cap at v', no floor), `ChipTarget`, `modulation_bands_for_session`, the two session/ sites (sim modulation, advisor) |

Measured (FM1): no recipe number moves; 88 cells carry
`chipload_point_mm` (75 Spektra, 5 SpeTool, 8 AMS-159); the 8 AMS-159
cells lose their zero-width band columns; the sim CSV power / deflection
peaks move with the modulated point feeds. The advisor optimises the
Spektra point cells (it skipped them before). `g_dcflat` is green.

Decisions taken on the way: the gate keeps no minimum for a point (the
retargeter and the ranking must not see a fake band); a pin arm on a point
keeps its own binding tag. Known: the AMS-159 V-bit gate reads another,
extrapolated row at the engaged width (B4). f036b AB5 ("never below the
minimum") is empty on a point and says so.
