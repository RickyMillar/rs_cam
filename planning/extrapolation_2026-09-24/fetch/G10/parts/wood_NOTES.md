# G10 fetch, part `wood`: notes

Vendors: LMT Onsrud, Amana Tool, Whiteside. Extras: Vortex, IDC Woodcraft,
ToolsToday, Freud, AXYZ. Access date: 2026-09-25.

Files: `parts/wood_sources.json` (15 stored sources, 4 dead ends),
`parts/wood_statements.json` (47 statements), `sources/wood_*.txt`.
The script `scripts/g10_wood_store.py` writes the text copies. The script
`scripts/g10_wood_build.py` writes the two JSON files and computes every
derived value. `g10_check_statements.py` gives 0 failures for this part.

The `value_si` field of this part uses these units: a feed in mm/min
(IPM x 25.4), a fraction without a unit, an angle in degrees. The `material`
field is an object `{class, label}`, where `label` is the text as printed.

## 1. Result per target

| Target | Result | Best evidence |
|---|---|---|
| 1. Maximum ramp angle (general, per flute count, centre cutting, diameter) | **Not published** by any wood vendor I could reach | No Onsrud, Amana, Whiteside, Vortex, Freud or IDC document prints a ramp angle. |
| 2. Helix entry: diameter as a fraction of D, pitch or helix ramp angle | **Not published** | Onsrud names "helical ramp with interpolation for holes" (plastics) and "helical interpolation" (composites) without numbers. IDC prints a spiral-drilling plunge feed, not a geometry. |
| 3a. Plunge feed against side feed | **Found** (hobby-grade wood chart) | IDC Woodcraft prints a Plunge column: fraction 0.05 to 0.57 across families (§2). |
| 3b. Ramp feed against side feed | **Found** (industrial wood charts) | Amana prints "To find Ramp Down: Feed Rate IPM / # of flutes" on six charts, and prints a Ramp Down column on three. |
| 4. Statements specific to wood, MDF, plywood | **Partly** | Amana columns per Wood, MDF/Laminate, Plywood; the ramp fraction does not change with material. Vortex: down cut "CANNOT be used to plunge straight into wood". Onsrud: ramp up and down in plywood and laminated MDF (oscillation). |
| 5. Families that must not plunge or ramp | **Partly** | Down cut: must not plunge (Vortex, twice). Compression: "best to ramp" (AXYZ, advice). PCD: "TIPICALLY CANNOT PLUNGE" (Onsrud). No source forbids plunge or ramp for V-bits; Amana and IDC give V-bits a ramp or plunge feed. Non-centre-cutting tools: no wood-vendor statement found. |

Summary of the printed ramp and plunge fractions (derived = printed ramp or
plunge / printed feed):

| Family | Source | Fraction | Grade |
|---|---|---|---|
| compression 1/2/3 flute | Amana v8 column | 1.0 / 0.5 / 0.334 | a |
| flat up/down cut 2/3 flute | Amana Spektra v24 column | 0.5 / 0.333 | a |
| ball nose 2 flute | Amana v7 rule only | 0.5 | b |
| bull nose (corner radius) 2 flute | Amana rule only | 0.5 | b |
| V-bit insert 1 flute, 40 to 110 deg | Amana v16 column | 0.5 (rule says 1.0) | a |
| V-bit insert 1 flute, 120 and 150 deg | Amana v16 column | 1.0 | a |
| V-bit insert 2 flute | Amana v16 column | 0.5 | a |
| flat down cut 1/8, 1/4 in | IDC Plunge column | 0.30, 0.43 | a |
| flat up cut 1/8, 1/4 in | IDC | 0.50, 0.50 | a |
| ball nose 1/8, 1/4 in | IDC | 0.25, 0.43 | a |
| V-bit 30/60/90/120 deg | IDC | 0.57 / 0.33 / 0.56 / 0.375 | a |
| tapered ball nose | IDC | 0.417 | a |
| surfacing 1 in | IDC | 0.047 | a |
| bowl bit (radiused) | IDC | 0.1875 | a |
| any | ToolsToday prose | about 0.5 | c |

## 2. What each source prints

- `wood_amana_compression_v8`: Ramp Down column (IPM) per diameter, flute
  count and material. 1 flute: ramp = feed. 2 flute: ramp = feed/2. 3 flute:
  ramp = feed/3, rounded (350 → 117, 300 → 100). Prints the rule. The chart
  gives compression bits a ramp-down feed. It does not forbid plunge.
- `wood_amana_spektra_plunge_v24`: the same form for 2 and 3 flute up-cut and
  down-cut flat spirals, Wood/Plywood and MDF/Laminate. The up-cut and the
  down-cut part number share one row, so Amana gives a down-cut bit the same
  ramp feed. This does not agree with the Vortex "no plunge" rule for down cut.
- `wood_amana_ball_nose_v7`, `wood_amana_corner_radius_plunge`,
  `wood_amana_spektra_engraving_v4`: the rule only, no column.
- `wood_amana_insert_vgroove_v16`: Ramp Down column for 36 V-groove inserts,
  six materials. See §5 for the conflict between the rule and the column.
- `wood_toolstoday_calc_video`: Amana distributor. It reads the chart value as
  "ramp-down or plunge rate" (46172-K compression, MDF, 260 → 130). It prints
  Feed and Ramp Down for five bits; all are feed/2, the 1-flute RC-1108 V-bit
  included (30 → 15).
- `wood_toolstoday_understanding_feeds_speeds`: "reduce ramp or plunge feed to
  about half the main feed rate". General advice, grade c.
- `wood_vortex_catalog`: "Downcut tools CANNOT be used to plunge straight into
  wood and should be ramped into the part." Series 1300: "Never plunge
  straight down with downcut tooling as this may cause fire or breakage." Up
  cut: "straight plunge/drill". Veining bits: "end cutting so they can plunge".
  No ramp angle or plunge rate.
- `wood_onsrud_routing_guide`: prose only. "Ramped Plunging into the
  workpiece" and "Higher Plunge Speeds" as heat reduction. Oscillation ramp in
  plywood and laminated MDF. PCD "TIPICALLY CANNOT PLUNGE".
- `wood_onsrud_pct19`: full catalog text. The only printed plunge feed is the
  34-100 potted-fastener tool for composite panels: 40 IPM plunge, 80 IPM feed.
  The 66-500 composite series: "Burr end for ramping and helical
  interpolation", "End mill point for plunging and helical interpolation".
  No wood entry data.
- `wood_onsrud_plastics_faq2`: plastics. "use a helical ramp with
  interpolation for holes"; "reducing plunge feed rate can reduce stress".
- `wood_idc_feeds_speeds`: benchtop wood chart ("soft, medium and moderately
  hard wood"). A Plunge (in/min) column next to Feed for every family. The
  1/8 in drilling endmill prints Plunge 60* at Feed 50 for "the spiral
  drilling technique" and 25 for conventional plunge.
- `wood_whiteside_cnc_brochure`: up cut "Best At Plunge Cuts". Nothing more.
- `wood_axyz_compression_bit_tip`: a machine builder. Up-cut tools can plunge;
  "for downward spirals and/or compression tools, it is best to ramp into the
  material".

## 3. Dead ends

| URL | HTTP | Result |
|---|---|---|
| https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf | 200 | Chip load chart only; no entry text. Raw PDF kept (`pdf/wood_freud_cnc_feed_speed_2017.pdf`). |
| https://www.vortextool.com/media/assets/chipLoadChart.pdf | 200 | Image-only page. I rendered it and read it: chip loads and RPM formulas only. |
| https://www.amanatool.com/46350-cnc-solid-carbide-mortise-compression-spiral-1-4-dia-x-1-inch-x-1-4-shank.html | 403 | Blocked. |
| https://www.amanatool.com/router-bit-technical-information | 403 | Blocked; the G4 Wayback copy has no entry text. |
| https://onsrud.com/technical-information/ , /tech-tips/ , /xdoc/TechInfo/ | 404 | No such pages. |
| https://onsrud.com/articles/*.asp (10 other articles: Router Way, Sign Industry, Spoilboards, Routing with Air, plastics articles) | 200 | Fixturing-and-Routing-of-Plastics has prose "Ramp Into Inside Cuts", circular ramp into a hole; no numbers. Not stored (plastics, repeats FAQ 2). The others have no entry text. |
| https://onsrud.com/images/{MDF,Hard Wood,Laminated Plywood}.pdf (stored under G2) | 200 | Chip load only; no entry text. |
| https://www.whitesiderouterbits.com/pages/faq , /pages/cnc-feeds-and-speeds , /pages/technical-information | 404 | The site has no technical pages (home page links: about, contact, tool files). |
| Whiteside full catalog 2027 (G1 copy) | — | "plunge" occurs only in product names (plunge roundover, plunge point). |
| https://www.vortextool.com/technical-information , /tech-info , /feeds-and-speeds | 404 | No such pages. |
| https://burnettetools.com/news/amana-tool-cnc-router-bits-review-sizing-and-chip-load-chart | 200 (WebFetch) | Prints "a ramp-in entry angle of no more than 10°" but cites no Amana document. A reseller blog. Not stored, so that nobody cites it as Amana. |
| https://camheads.org/archive/index.php/t-6807.html | 200 (WebFetch) | Search summary said Vortex prints "45 degree angle ... 1/3 of the calculated feed rate". The page does not contain it; the Vortex chart does not contain it. Unverified; not stored. |

## 4. Searches

1. `Onsrud ramp angle router bit ramping recommended degrees`
2. `Amana Tool ramping CNC router bit ramp angle helical interpolation recommendation`
3. `Whiteside router bits CNC ramp plunge compression bit "ramp" recommendation`
4. `onsrud.com tech tip ramping plunging "ramp" spiral router bit`
5. `compression router bit do not plunge "ramp" onsrud OR amana OR vortex "upcut portion"`
6. `toolstoday ramping vs plunging CNC router bit ramp angle degrees blog`
7. `Vortex Tool router bit feeds speeds plunge rate ramp "vortextool"`
8. `IDC Woodcraft ramp angle recommendation compression bit ramp plunge`
9. `Onsrud compression spiral "plunge" "ramp" upcut length mortise compression tip technical`
10. `Amana Tool compression bit "ramp" into cut plywood recommendation "mortise compression"`
11. `CMT Orange Tools CNC router bit plunge feed rate chart "plunge" "ramp"`
12. `router bit helical ramp entry hole diameter "times the tool diameter" wood CNC ...`
13. `Bits & Bits CNC router bit feeds speeds chart plunge rate ramp angle`

I also grepped the repo copies named in the task, the full Onsrud PCT-19
text, the Whiteside catalog and brochure, and the Onsrud material PDFs.
CMT and Bits & Bits gave no vendor document with entry data in the searches;
I did not crawl their sites. The Adam's Bits guide (stored under G6) prints
"You can plunge at 800 mm/min for timber"; I left it to the `hobby` part.

## 5. Items for the verifier

1. **Amana V-groove rule against column.** The v16 footer prints "To ﬁnd Ramp
   Down: Feed Rate IPM / # of Flutes)". Every 1-flute row from 40 to 110
   degree prints Ramp = Feed/2 (for example RC-1108: 40 → 20). Only RC-1146
   (120 deg) and RC-1101 (150 deg), both at 14,000 RPM, print Ramp = Feed.
   ToolsToday prints RC-1108 at 30 → 15. So the printed V-bit fraction is 0.5,
   and the rule would give 1.0. The text copy carries the ligature U+FB01.
2. **Meaning of "Ramp Down".** Amana never defines it. It can be a Z plunge
   feed or the feed along a ramp. ToolsToday says "ramp-down or plunge rate".
   A ruling must choose whether it fills `plunge_rate`, the new ramp feed, or
   both.
3. **The Amana ramp fraction depends only on the flute count.** The rule and
   the columns do not change with material or diameter. For 1-flute
   compression bits the ramp equals the side feed (fraction 1.0).
4. **Down cut conflict.** Amana gives down-cut bits the same ramp-down feed as
   up-cut bits (one shared row). Vortex forbids a straight plunge with down
   cut. These can both be true only if "Ramp Down" is a ramp feed.
5. **IDC columns.** The V-bit table has a "Side Angle" column between Shank
   and Feed; I read Feed and Plunge after it (30 deg: 15, 35, 20). The IDC
   starter-set table on p3 prints the 60 deg V-bit at Feed 40, and section 2
   prints Feed 60, both with Plunge 20. The taper ball nose "Cut Dia." 0.250
   is not the tip size; the tip radius is 0.015 or 0.045 in.
6. **Layout splits.** The Spektra v24 rule sits on two lines with table
   cells between them. The statement verbatim is only the formula part.
7. **Onsrud PCD** text prints "TIPICALLY" and "CAPABIITIES" as spelled.
