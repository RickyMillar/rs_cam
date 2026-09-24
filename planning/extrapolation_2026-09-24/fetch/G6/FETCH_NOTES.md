# G6 fetch notes: drill chip load, plunge rate, peck depth

Date: 2026-09-24. Agent: G6 research (Phase 1). No cargo, no LUT edit.

## 1. Result in one table

| Target | Result | Best evidence |
|---|---|---|
| 1. Drill chip load in wood, MDF, particleboard, plywood | FOUND for **wood drills** (brad-point, dowel, twist, Levin). Not for router end mills. | Onsrud 72-000 (printed chip load band; family mapping derived, grade b); Leitz Lexicon ch.6 (printed vf at n, chip load derived); CMT 311.71/72HWM (prose, derived) |
| 2. Plunge rate for a router end mill | FOUND, one chart column and one product page | Amana Spektra v24 "Ramp Down" column and rule `Feed Rate IPM / # of flutes`; PreciseBits fret-plane plunge IPM by Janka |
| 3. Peck depth in wood | PARTLY FOUND. Printed total-hole regimes and one maximum infeed. No per-peck depth for a router end mill. | Leitz Lexicon ch.6 (4 x D before a clearance stroke; 2 x D maximum infeed for boring pins in hardwood) |

Files: `sources.json` (13 sources), `candidate_rows.json` (31 rows and 8
depth statements), `lut_discrepancies.md` (none), stored texts in
`sources/`. The generator `scripts/g6_candidate_rows.py` asserts that every
`verbatim` string occurs in its stored text. `scripts/g6_leitz_red_points.py`
reads the Leitz red worked-example values from the PDF text layer.

## 2. The mapping question the reconciler must answer first

The 80 refused G6 cells are **router tools** (`EndMill` 3.175 and 6.0 mm) on
the `Drill` and `AlignmentPinDrill` operations. They are not drill bits.

- The Onsrud 72-000, Leitz and CMT rows are for **wood drills** (brad point,
  dowel, twist, Levin). They fill a drill *tool*. Do not read them onto an
  end-mill plunge. The rows carry `tool_family` `brad_point_drill` or
  `twist_drill` or `levin_drill`.
- The Amana "Ramp Down" rows and the PreciseBits rows are for **router end
  mills**. Only these rows sit on the refused cells.
- The LUT `ToolFamily` enum (`crates/rs_cam_core/src/feeds/vendor_lut.rs:102`)
  has no drill arm. `brad_point_drill`, `twist_drill` and `levin_drill` are new values.
  They need an enum arm before any row loads. No row is `exact`/`a`:
  every row either converts units with an RPM or maps a shared material
  column to one family. `operation_family` "drill"
  exists in `feeds::OperationFamily` but no LUT row uses it yet.

## 3. What each source prints

### 3.1 Onsrud Drill.pdf (derived grade b, 4 rows)

- Printed: `72-000* Wood` chip load per tooth `3 mm .009-.011`,
  `5 mm .011-.013`, `6 mm .013-.015`, `8 mm .015-.017` in.
- Footnote: `* Gang drills run at 4,500 RPM and 150 IPM`. This confound
  travels in every row's notes: a rigid multi-spindle borer, not a router
  collet. 150 / (4500 x 2) = 0.0167 in/tooth (derived) is inside the 8 mm band.
- Flutes = 2 is printed in the PCT-19 catalogue, page 90.
- The sheet prints one "Wood" row. The rows use `hardwood` as a placeholder.
  By the R5 rule a shared column is derived grade b for each family, so the
  rows are derived grade b although the band itself is printed.
- Same bytes as the 2026-08-04 audit (756 398 bytes). The audit's d^0.485
  midpoint fit still applies.

### 3.2 Leitz Lexicon Edition 7, chapter 6 Drilling (grade b, 13 rows)

- Every tool page prints a diagram "Feed speed vf depending on the spindle
  RPM n". It prints one red worked example as text (vf in m/min, n in 1/min),
  Z in the heading, a base material and correction factors. It prints **no
  chip load**. Every row's chip load is derived: vf x 1000 / (n x Z).
- The plotted bands are graphics. G6 did not transcribe the band edges.
  A later reader can digitise them from the PDF (they are vector paths).
- PDF page = printed page + 2.
- Dowel drills (all six diagrams, Z2, D 3-10 mm): 2 m/min at 4500 on
  chipboard plastic coated gives 0.222 mm/tooth. The printed correction
  "MDF, solid wood = 0.7" gives 0.156 mm/tooth, and "Chipboard, uncoated =
  1.3" gives 0.289 mm/tooth (both derived twice). The base row is labelled
  coated board; the uncoated row is the raw particleboard figure.
- "Solid wood" (no softwood/hardwood split) uses `softwood` as the placeholder
  on every Leitz row.
- Twist drills in softwood (Z2): 1.2-4.5 m/min at 2500-4500 gives
  0.13-0.50 mm/tooth. Hardwood corrections 0.7 (HS) or 0.8 (HW) are printed.
  Laminated veneer lumber corrections 1.1-1.2 are printed.
- Levin drills (Z1), solid wood: 1.5 m/min at 4500 gives 0.333 mm/tooth.
- Two diagrams (PDF p45-46, cylinder head drills, hardwood) print their red
  values as graphics, not text. G6 made no row from them.

### 3.3 CMT 311.71/72HWM product page (grade b, 2 rows)

- Printed: `Recommended feed speed 1÷ 4m/minute – RPM 6000.`, `2 cutting
  edges [Z2]`, `Ideal for chipboard, MDF, HDF and laminates.`
- Derived: 0.083-0.333 mm/tooth. D 5-10 mm.
- The page contradicts itself on the point ("No center-point or spurs" and
  "2 curved ground spurs [V2]"). The note records this.

### 3.4 Amana Spektra v24 "Ramp Down" (grade b, 8 rows)

- Printed per row: `Feed Rate IPM`, `Chip Load Per Tooth`, `Ramp Down` for
  Wood/Plywood and MDF/Laminate. Printed rule: `To find Ramp Down: Feed Rate
  IPM / # of flutes`. Printed spindle speed: 18,000 RPM.
- Amana does not print the word "plunge" for this column. The rows keep the
  printed name "Ramp Down". ToolsToday (an Amana retailer, grade c) writes
  "reduce ramp or plunge feed to about half the main feed rate".
- Derived: axial advance per tooth = Ramp Down / (18000 x flutes) = the side
  chip load / flutes. 2F: one half of the side chip load. 3F: one third.
  The rows put this derived value in `chipload_max_mm_tooth` and the printed
  IPM in `ramp_down_ipm`.
- Rows cover the matrix sizes only (1/8 in and 6 mm, 2F and 3F). The chart
  prints the same rule for every size from 1/32 in to 3/4 in.
- The Amana ball nose v7 chart prints the same rule with no column.

### 3.5 PreciseBits fret-plane page (grade b, 4 rows)

- Printed: plunge rate by wood hardness, softwood (Janka < 1,500) 75 in/min,
  medium hardwood (1,500-2,500) 50 in/min, high hardness (> 2,500) 40 in/min.
- One tool: 3-flute radiused end mill, 3.18 mm, tip radius 0.64 mm.
- No RPM is printed. The rows carry `plunge_feed_mm_min_printed` and a null
  chip load.
- The page bands by Janka. The engine's generic hardwood (Janka 1450) is in
  the < 1,500 band, so it reads 75 in/min, not 50. The rows carry
  `hardness_value` 600 and 1450 (75 in/min), 2000 (50) and 2600 (40).

### 3.6 Printed refusals (0 rows, refusal reasons)

- Freud 2017: "Carbide tipped bits should not be used to drill directly into
  the work piece."
- Whiteside catalogue, carbide-tipped boring bits 6100 and 6140 (not the
  dowel drills): "NOT for use in routers."
- Leitz, boring pins in hardwood and glulam: "Interim chip removal (return
  stroke) then is obligatory."

### 3.7 Prose ratios (grade c, 0 rows)

| Source | Printed text | Ratio |
|---|---|---|
| CLE Bit Co. | "Plunge the material at a 45 DEG angle over 2-4 inches if possible at ⅓ of the calculated feed rate." | 1/3, and it is a ramp, not a plunge |
| ToolsToday | "many CNC users reduce ramp or plunge feed to about half the main feed rate" | about 1/2 |
| Adam's Bits | "You can plunge at 800 mm/min for timber and plastic" ... "The smaller the bit, the lower the plunge rate." | absolute, no size |
| Onsrud 34-100 (composites) | "RPM 10,000 / Plunge Feed Rate 40 IPM / Feed Rate 80 IPM" | 1/2, composite honeycomb, not wood |

## 4. Peck depth (target 3)

The 2026-08-04 audit found no printed peck guidance. Leitz prints four
statements. They are in `candidate_rows.json` `depth_statements` with
verbatim text:

1. Twist drill, HW Z2/V2 with heel (PDF p36): "When drilling holes with a
   depth greater than 4 x D interim clearance stroke is recommended!"
2. Levin drill, HS Z1 (PDF p43): "Suitable for depths up to approx. 4 x D
   without interim clearance strokes." Feed correction "Drilling depth > 4 x
   D = 0.8".
3. Levin drill, HW Z1/V1, D 12-16 (PDF p44): "Suitable for depths up to 75 mm
   without interim clearance strokes."
4. Boring pins (PDF p13): "Infeed depth in hardwood and glulam maximum 2 x D."
   and a return stroke is obligatory in hardwood and glulam.

Also printed: some HW twist drills are sold "for drilling deep holes in solid
wood without interim clearance strokes"; and a dwell warning, "Tool too long
at the reversal point" makes chips and workpiece hot.

What these are: a **total-hole regime** (about 4 x D before the first
clearance stroke) and a **maximum infeed** (2 x D for boring pins in
hardwood). They are for wood drills. None states a per-peck depth for a
router end mill. The repo's per-peck ceilings (6 / 5 / 4 x D by Janka) still
have no source. The printed 4 x D regime is below the repo's 5 x D hardwood
per-peck ceiling and the 6-8 x D chip-welding bands.

## 5. Derived cross-checks (not claims)

- Wood-drill chip load, three vendors at 4500-6000 RPM:
  Leitz dowel 0.222 mm/tooth (chipboard coated), 0.156 (MDF / solid wood);
  Onsrud 72-000 0.229-0.432 mm/tooth; CMT 0.083-0.333 mm/tooth. The ranges
  overlap. This is a possible second witness for a wood-drill claim.
- Tension for Phase 2: Leitz plots one vf band for dowel drills D 3-10 mm
  (feed independent of D, so constant mm/tooth). Onsrud's midpoints rise as
  about d^0.485. G6 does not resolve this.
- End-mill plunge: Amana prints plunge-direction advance per tooth = side
  chip load / flutes (a factor of 0.5 or 0.33). The repo's
  `DRILL_CHIPLOAD_MULTIPLIER` is 2.5, and it goes the other way. The 2.5
  factor is the wood-drill direction (the audit found about 5 against
  Onsrud). An end mill and a drill bit need different answers.
- Absolute end-mill plunge, 1/8 in at 18,000 RPM: Amana Wood/Plywood
  72.5 in/min (2F) and 72 in/min (3F); PreciseBits softwood 75 in/min (3F,
  no RPM stated). The numbers are close. Different tools and conditions.

## 6. What G6 searched

### Queries (WebSearch)

- `Onsrud 72-000 series drill wood boring bit`
- `boring bit feed per revolution chart wood MDF particleboard CNC pdf`
- `Amana boring bit recommended RPM feed rate chart dowel drill`
- `Leitz Lexicon dowel drill feed per revolution recommended cutting speed particleboard`
- `dowel drill "feed per revolution" "mm/rev" chipboard recommended`
- `Freud dowel drill boring bit speed feed recommendations pdf`
- `router bit plunge rate recommendation "plunge" feed rate percent of feed rate Onsrud OR Vortex OR Whiteside chart`
- `Onsrud tech tips plunge feed rate spiral router bit`
- `Vortex Tool feeds and speeds plunge rate chipload pdf`
- `vortextool.com "plunging" "1/3" feed rate 45 degree angle`
- `Whiteside Machine router bit "plunge rate" recommendations CNC`
- `Guhdo OR "Fisch" OR Famag dowel drill Schnittwerte Vorschub pro Umdrehung Spanplatte`
- `CMT industrial catalogue dowel drills "feed" "m/min" rpm chipboard technical information`
- `Guhdo Dübelbohrer Katalog pdf Drehzahl Vorschub m/min Diagramm`
- `brad point dowel drill CNC recommended feed "mm/rev" OR "per revolution" wood manufacturer technical data`
- `amanatool.com pdf "boring bits" feed rate chipload drilling wood CNC`
- `Whiteside dowel drill feeds speeds chart pdf`
- `Diablo OR Colt OR Famag Forstner brad point bit recommended RPM feed chart wood`
- `drilling wood CNC peck depth "x diameter" chip clearance brad point dowel drill manufacturer recommendation`
- `toolstoday feeds and speeds chart plunge rate router bit "plunge" "ramp"`
- `precisebits reference plunge feed rate end mill wood "plunge"`
- `cmtorangetools.com pdf industrial catalogue dowel drills "Recommended feed speed"`

### Dead ends

| Vendor / URL | Result |
|---|---|
| Amana product pages (`amanatool.com/...boring-bit...html`, countersink page) | HTTP 403 for curl and WebFetch. Search snippets say "max RPM 8,000" (boring bits) and "3,000 RPM with a 30 IPM plunge rate" (countersinks). Not stored, so not cited. |
| Guhdo catalogue `guhdo.de/sites/default/files/2019-05/Guhdo_Kat2019_K5_Bohrer_low_1.pdf` | HTTP 404. The catalogue page `guhdo.de/en/kataloge/` has no download links (request only). |
| Vortex catalogue `ctsaw.com/wp-content/uploads/2015/03/Vortex_Catalog-2019.pdf` | HTTP 403. `vortextool.com/feeds-speeds` is a router-bit calculator with no drill or plunge data. `vortextool.com/chip-load-chart` and `/technical-information` are 404. |
| CMT 2014 leaflet `scosarg.com/media/leaflets/cmt/2014_CNC_Tools.pdf` | HTTP 403. |
| CMT 311.41/42 HW, 314.21/22 HWM, 308 HW pages | 200, but no feed printed. |
| Fisch `fisch-tools.com/en/products/dowel-drill` | Product list only; no feed or speed. |
| Whiteside catalogue | Dowel drills and boring bits listed; no feeds. |
| Onsrud `onsrud.com/Series/72-000.asp` | Product index page; no data beyond the catalogue. |
| Onsrud PCT-19 technical pages 109-112 | No plunge or drill guidance for wood. |
| PreciseBits `reference/drillfeedspeed.htm` | Carbide PCB drills in FR-4, not wood. |
| Diablo Forstner chart | RPM only; not a CNC tool. |
| Famag, Colt, Fisch | No printed feed or chip load reached. |
| ToolsToday drill-geometry page | Qualitative only. |
| ShopBot feeds PDF | No plunge or drill content. |

## 7. What G6 could not find

- A printed chip load, or plunge chip load, for **drilling with a router end
  mill** (a straight Z plunge to depth). Amana's "Ramp Down" is the closest
  printed figure. It is named "Ramp Down", not "plunge".
- A printed **per-peck depth** for any tool in wood. Leitz prints a
  total-hole regime (4 x D) and a maximum infeed (2 x D, boring pins). No
  vendor reached prints a per-peck depth for a router end mill.
- Printed chip loads for Amana, Freud, Guhdo, Vortex, Whiteside, Fisch, Colt,
  Famag or Diablo boring bits. Not published anywhere G6 could reach.
- A source for `DRILL_CHIPLOAD_MULTIPLIER = 2.5`. It stays unsourced.
