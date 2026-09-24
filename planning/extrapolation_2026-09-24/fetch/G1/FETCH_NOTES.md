# G1 fetch notes: size (sub-1.5 mm and multi-size series)

Date: 2026-09-24. Author: the G1 research agent (Phase 1 of the extrapolation
programme). No cargo, no LUT edit, no commit.

## 1. Result in one paragraph

Two vendors print wood chip loads for tapered ball carving bits under 1.5 mm,
keyed on the tip. **Amana** prints them on the ZrN 2D/3D carving chart, which
the LUT already cites. The LUT files those cells as `ball_nose`, so the
tapered lookup never sees them (`lut_discrepancies.md` D1). **SpeTool** prints
a 9-size tapered chart from 0.5 mm to 4.0 mm. Both charts print one
"Wood, MDF, Sign-Foam" row, so hardwood is not printed apart. For the
acceptance case (1.0 mm tip, 2 flutes, hardwood) the printed figures are:

| Source | Cell | Chip load (printed) | mm/tooth (x 25.4) |
|---|---|---|---|
| Amana ZrN v8 | 2 Flute Ball Nose, 1mm (tool 46256, 5.4° taper, 1 mm tip) | 0.00075" - 0.002" | 0.019 - 0.051 |
| SpeTool 2D/3D | Tip Diameter 1.0 mm | 0.001 | 0.0254 |

Both are "Wood, MDF, Sign-Foam" figures. A hardwood claim from them needs a
ruling on the unsplit "Wood" row. That is a G2 question. Two precedents
disagree:

- R5 (feeds matrix, 2026-09-23, operator-ruled) applies a shared chart column
  to each wood family it covers as `derived`, grade b (the Spektra
  "Wood/Plywood" column gives hardwood rows).
- The manifest note on this same Amana chart (2026-05-02) says: "HARDWOOD AT
  SUB-1MM IS A DOCUMENTED GAP ... per refusal-first we do not invent
  reduced-CPT hardwood rows".

The candidate rows follow R5 (MDF `exact`, softwood and hardwood `derived`,
grade b), because R5 is later and ruled. The reconciler picks between the
two. Deleting the 18 hardwood rows applies the older note.

## 2. Sources stored and rows written

| source_id | Rows | What it gives |
|---|---:|---|
| `amana_zrn_3d_profiling_v8` | 27 | tapered ball, tip frame: 2 flute at 1.0, 1.5875, 6.35 mm; 3 flute at 0.794, 3.175, 4.7625 mm; 4 flute at 1.5, 1.5875, 3.175 mm |
| `spetool_2d3d_tapered_router_bit_chart` | 27 | tapered ball, tip frame, 2 flutes (derived): 0.5, 0.794, 1.0, 1.5, 1.5875, 2.0, 3.0, 3.175, 4.0 mm; one value each |
| `spetool_carbide_spiral_router_bit_chart` | 42 | flat spiral 1/16 to 1/2 in; hardwood, softwood, MDF apart; up, down, compression (`exact`, grade b: a scan with a derived unit and flute count) |
| `amana_zrn_3d_profiling` (v1) | 0 | same file as the LUT copy (same sha256) |
| `amana_46xxx_tool_identity` | 0 | the geometry of each tool number on the Amana chart |
| `spetool_vgroove_signmaking_chart` | 0 | two engraving tip widths with the same chip load; no size series |
| `spetool_speed_feeds_index` | 0 | ties the 2D/3D chart to "2D & 3D Tapered Router Bits" |
| `harvey_tapered_ball_SF_723300` | 0 | metals only; a frame statement |
| `precisebits_sweetspot_test` | 0 | a start rule for a feed test, not a value |
| `precisebits_tapered_ball_carving_250_shank`, `precisebits_tapered_ball_carving_125_shank_4f` | 0 | no chip load; "Do the Sweetspot Test" |
| `precisebits_tapered_stub_pcb` | 0 | PCB and metal, not wood; a frame statement |
| `onsrud_77100_series_list` | 0 | confirms the 77-100 flute counts; no tip under 1/8 in |
| `shopbot_feeds_speeds_2016` | 0 | a reprint of Onsrud with a wrong flute count |
| `whiteside_cnc_brochure_1-14-19`, `whiteside_full_catalog_2027` | 0 | no chip load; SC64/SC66 geometry and flute heading |
| `idcwoodcraft_chipload_csv_2026-09-24` | 0 | community presets with no material (grade c at best) |

Total: 96 candidate rows. Every row has `extrapolation_group: "G1"` and a
`verbatim` line. The script `scripts/g1_candidate_rows.py` asserts that each
verbatim line is in its stored text and that each printed feed closes
`feed = RPM x flutes x chip load`.

Two of the stored PDFs (SpeTool) are scans. Their stored text has a
transcription from the page image (section A) and the raw OCR layer
(section B). I checked section A against section B cell by cell.

## 3. The diameter each chart keys on (target 4)

| Source | Key | Printed text |
|---|---|---|
| Amana ZrN chart | the tool "Dia.", which is the ball tip of a tapered tool | "Tool Reference #'s ... 46256 1mm Dia."; the product spec prints "Diameter (D) 1mm, Radius (R) 0.5mm, Angle (a°) 5.4°" |
| SpeTool 2D/3D chart | the tip | column header "Tip Diameter (MM)" and "Tip Diameter (INCH)" |
| Harvey tapered ball (metals) | the tip | "Use the end diameter of the tool to select the correct Chip Load (IPT)" |
| PreciseBits tapered tools | the tip | "Diameter (D) = tip diameter" (tapered-stub page); stepdown "1X tip dia." (carving page) |
| Onsrud 77-100 (LUT rows) | the tip | the sheet's "cutting diameter" of a taper tool is the ball tip (LUT row note) |

No source that I found prints a chip load keyed on the engaged diameter or on
the top of the cone. The engine scales by the engaged diameter at the cut
depth. A law fitted on these rows is a law in the tip frame. Using it at the
engaged diameter mixes two frames.

Two related facts that the charts print:

- Amana gives one value to a tapered and a straight ball of the same tip
  (2 flute 1/4": 46283 at 3° and 46294 / 46479 at 0.10°; 3 flute 1/8": four
  tapers from 1° to 7° and the straight 46295). On this chart, the taper
  angle does not change the value at a given tip.
- The Amana 4 flute section prints one value (0.0005" - 0.00065") at every
  size from 1.5 mm to 1/4". That is a flat series (no size effect).

## 4. Size shape of the new series (derived, for Phase 2 only)

These ratios are my arithmetic on printed values. They are not fits and they
are not in any row.

- SpeTool tapered, 0.5 to 4.0 mm (8x): 0.0007 to 0.005 (7.1x).
- Amana tapered 3 flute, 0.794 to 4.7625 mm (6x): band midpoint 0.001375 to
  0.00325 (2.4x).
- Amana tapered 4 flute, 1.5 to 3.175 mm: constant.
- Amana tapered 2 flute, 1.0 to 6.35 mm (6.35x): band midpoint 0.001375 to
  0.008 (5.8x). The 1/16" cell in between is self-inconsistent (see below).

So the vendors do not agree on one exponent for tapered balls, even in the
tip frame. Phase 2 must fit per chart and state the spread.

## 5. Chart defects that the rows carry in their notes

- Amana 2 flute 1/16": the printed chip load (0.003" - 0.005") is twice the
  printed IPM (55" - 90" at 18,000 x 2 gives 0.0015" - 0.0025"). The rows keep
  the printed value at grade b.
- Amana 3 flute 1/8"-3.2mm and 3/16": the printed IPM gives a lower top of
  the band than the printed chip load (0.00185" against 0.0025"; 0.00315"
  against 0.004").
- Amana lists 46471 twice: as "1mm Dia." in the 2 flute section and as
  "0.8mm Dia." in the 3 flute section. The product page says 1 mm, 0.10°.
- Amana v8 lists 46473 (0.5 mm tip) in the 3 flute section, but no column
  covers 0.5 mm (the column label starts at 1/32"). I wrote no 0.5 mm row
  from Amana. SpeTool is the only printed 0.5 mm wood figure.
- SpeTool prints no unit and no flute count. Feed / (RPM x chip load) = 2.00
  on all 9 tapered wood rows, so the rows use inch per tooth and 2 flutes
  (derived). The spiral chart rounds its feeds (the identity gives 1.94 to
  2.13).

## 6. What I searched

Web searches (WebSearch):

1. `Amana 46282 tapered ball nose 1/16 D 4 flute 3D carving` → distributor
   pages; confirmed Amana 46xxx carving bits are tapered.
2. `tapered ball nose chipload chart wood pdf tip diameter 0.5mm 1mm` → Amana
   46256 product page (tapered, 1 mm tip), SpeTool W01010.
3. `Onsrud 77-100 tapered ball nose 1/32 1/16 cutting data chipload` →
   onsrud.com series and product pages (no new sizes).
4. `Amana 46298 3/16 ball nose carving`, `Amana 46494 OR 46495 ...`,
   `Amana 46580 1/32 carving tapered ball nose` → toolstoday and arrowtooling
   product pages for numbers that fastoolnow did not return.
5. `Carbide3D feeds and speeds chart #112 1/16 #122 1/32 hardwood softwood MDF`
   → community threads only; no printed Carbide 3D tapered chart.
6. `SpeTool tapered ball nose feed speed chart pdf wood chipload` →
   https://spetools.com/pages/spetool-router-bits-speed-and-feeds (the PDFs).
7. `Harvey Tool miniature ball end mill wood speeds and feeds pdf` → Harvey
   tapered ball product page; 16 speed and feed PDFs.
8. `Bits & Bits tapered ball nose recommended feeds speeds chipload ...` →
   forum threads; no Bits & Bits chart.
9. `"tapered ball nose" "chip load" chart hardwood 0.25mm 0.5mm 1mm ...` →
   Freud 72-300, toolstoday category, a Burnette Tools blog (a distributor
   summary, not used).
10. `Whiteside tapered ball nose SC chip load chart router bits wood` →
    Whiteside SC64 page, brochure and catalog.
11. `Freud 72-300 72-302 tapered ball tip CNC bit feed speed ...` → Freud
    product pages.
12. `docs.carbide3d.com feeds speeds chart tapered ball #202 #201 wood` →
    community threads and an old Shapeoko image; no tapered chart.
13. `micro end mill wood chipload chart 0.5 mm 0.8 mm 1.0 mm diameter hardwood
    pdf router` → the ShopBot chart; calculators.
14. `Datron wood milling tool feed per tooth table small diameter ball end mill
    wood` → Datron shop pages; no wood table.

Direct fetches (curl) and what they returned:

- https://www.amanatool.com/pub/media/productattachments/ZrN-3D-Profiling-Feed-Chip-Load-Chart.pdf
  and `...-v8.pdf`: both PDFs downloaded (200). Stored.
- https://www.amanatool.com/catalogsearch/result/?q=<n>: Cloudflare challenge
  ("Just a moment..."). Dead end; I used distributor pages.
- https://fastoolnow.com/search.php?search_query=<n> and the product pages:
  spec blocks for 36 of 41 tool numbers.
- https://www.precisebits.com/products/carbidebits/taperedcarve250b2f.asp,
  `taperedcarve125b4f.asp`, `tapered_stub_125.asp`,
  https://www.precisebits.com/tutorials/calibrating_feeds_n_speeds.htm: stored.
  PreciseBits prints no chip load chart for wood. It sends the user to the
  sweet-spot test. The earlier G1 run also saved the PreciseBits calculator,
  chipbreaker and diamondcut pages (`pdf/`); they are PCB router pages with
  no wood figure, and I did not store them.
- https://harveyperformance.widen.net/.../SF_*.pdf (16 tapered ball sheets):
  metals only, no wood or plastic row in any of the 16.
- https://onsrud.com/Series/SolidCarbideTaperedBallNose.asp: the product table
  loads by script; https://onsrud.com/Products/77102.asp embeds the series
  JSON, stored. https://onsrud.com/Series/77-100.asp: 404.
- https://www.whitesiderouterbits.com/products/sc64: links a "CNC Profile" PDF
  (a drawing, no feeds), the CNC brochure and the full catalog. No chip load
  chart in any of them.
- https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf:
  no ball or tapered rows. https://www.freudtools.com/products/72-300 (and
  72-301, 72-400): the content loads by script; WebFetch returned only the
  title. Dead end.
- https://bitsbits.com/product-category/cnc-router-bits/tapered-cnc-router-bits/:
  no chip load on the page.
- https://toolstoday.com/ball-nose-conical-ball-solid-carbide-spiral-cnc-2d3d-carving-tapered-and-straight-up-cut-router-bits.html:
  Amana's generic text ("run a high chip load in the order of .005" and up"),
  about plastics. Not a chart.
- https://idcwoodcraft.com/pages/chipload-calculator and
  https://feeds-speeds-chipload-api.fly.dev/download-csv: community presets.
  The IDC tapered presets give the same 0.00158 in/tooth at tips from 0.76 to
  2.29 mm (derived from feed, RPM and flutes). Grade c; no material; no rows.
- https://shopbottools.com/wp-content/uploads/2024/01/FeedsandSpeeds.pdf: an
  Onsrud reprint. Stored for the flute-count check.

## 7. Targets not found

1. **A printed tapered-ball chip load that separates hardwood from softwood
   under 1.5 mm.** Amana and SpeTool print one "Wood, MDF, Sign-Foam" row.
   Not published anywhere I could reach.
2. **Onsrud 77-100 below 1/8 in.** The series has no tip under 1/8 in, and
   the sheets print 1/8 and 1/4 in only.
3. **PreciseBits micro tapered or micro end mill chip loads for wood.**
   PreciseBits publishes a test method and a 3 % of D start rule, not values.
4. **Whiteside, Freud, Carbide 3D, Bits & Bits, Kyocera SGS, Datron: a wood
   chip load chart for tapered or micro tools.** None found. Whiteside and
   Freud publish geometry only for their tapered bits. I did not find a
   Kyocera SGS wood chart (their micro charts are for metals).
5. **Harvey Tool wood or plastic charts for miniature ball and tapered ball.**
   The 16 tapered-ball sheets print metals only.
6. **A micro straight ball (not tapered) under 1.5 mm in wood.** Only the
   Amana 46471 (1 mm, 0.10°) shares the 2 flute 1 mm cell with the tapered
   46256. No other printed wood value found.
7. **A micro flat end mill under 1.5 mm in wood, apart from the Spektra rows
   the LUT holds.** Amana prints a 3 flute "Flat Bottom" 1/32"-1mm cell
   (0.0003" - 0.0005"), but its tools (46570, 46571, 46581) are carving bits
   whose geometry I did not identify. Not transcribed.
8. **A bull nose size series.** None found. The LUT still has 0 bull series.
9. **A V-bit size series of 3 or more sizes with one flute count.** None
   found. SpeTool prints two engraving tip widths with the same value.
10. **Amana tool numbers 46285, 46289, 46496, 46593, 46596.** Not identified
    (distributor search empty).

## 8. Notes for the reconciler

- The PreciseBits sweet-spot page prints, verbatim: "Softwoods like pine or
  fir: F = 0.03 x D x No. flutes x RPM (3% chipload per flute)", and the same
  line for hardwoods and extreme hardwoods. It is a start value "just below
  the sweetspot" for a test, with D the tool diameter. It is linear in D and
  does not change with hardness. It is not a recommendation.
- The PreciseBits tapered-stub page (PCB and metal) prints feed per
  revolution linear in the tip diameter (0.00254 in/rev at 0.25 mm to 0.00634
  at 0.63 mm, 3 flutes). Not wood; a frame witness only.
- The SpeTool spiral rows are also G2 evidence (hardwood, softwood and MDF at
  five sizes) and G4 evidence (one value, no band).
- The SpeTool V-groove chart prints hardwood 0.003, softwood 0.005 and
  MDF/Laminate 0.007 at both engraving tip widths. That is a G2 category-ratio
  witness for engraving tips.
- Grade rule for a self-inconsistent Amana cell: where the printed chip-load
  band and the band from the printed IPM overlap, the row keeps its grade;
  where they do not overlap (only the 2 flute 1/16" cell), the row is grade b.
- `pdf/` holds the raw downloads and is not committed. The sha256 in
  `sources.json` lets a later run check a fresh download.
