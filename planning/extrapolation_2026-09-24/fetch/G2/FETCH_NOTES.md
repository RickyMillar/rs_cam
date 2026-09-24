# G2 fetch notes: material category

Date: 2026-09-24. Author: G2 research agent (Phase 1 of the extrapolation
programme). Status: fetch complete. No LUT, manifest or Rust file changed.
No cargo run.

Gap group G2: a cell refuses because the LUT has a row for the tool in
another wood category only. INVENTORY §1 counts 98 refusals: V-bit in MDF
and plywood (64), ball nose in plywood (24), ball nose in MDF with the
formula under 0.5x of the chart (10).

## 1. Results per target

| Target | Result |
|---|---|
| 1a. Ball nose in plywood | **Not published anywhere I could reach.** No vendor chart prints a plywood row for a ball nose. |
| 1b. Ball nose in MDF | Found. Amana PCD ball nose v2 (MDF and uncoated chipboard, 1/4 and 3/8 in). Amana Spektra 3D carving v6 ("Wood, MDF, Sign-Foam", 6 tools). Amana ball nose v7 (MDF at 7 sizes; the LUT holds 2). |
| 1c. V-bit in MDF and plywood | **Found, in charts the LUT already stores.** All Onsrud wood sheets print the 37-series (V bottom, lettering, engraving) with one band per size. The LUT never transcribed them. |
| 2. Vendor statements on plywood / MDF vs solid wood | Found for MDF and chipboard (Leitz correction factors, grade a text). **No vendor prints a plywood factor.** Machine builders group plywood with solid wood (Techno CNC, Sienci). |
| 3. Flat-end series held in all five categories | Listed in §5. Twelve tool/size keys; six Spektra keys have only two printed columns. |
| Janka proxies (MDF, plywood, particleboard) | Particleboard: a standards minimum exists (2,225 N = 500 lbf). MDF: **no published hardness found**; neither 700 nor 1100 has a source I could reach. Plywood: **none**; the proxy is the face species. |

## 2. Candidate rows (candidate_rows.json, 101 rows)

All rows are `row_kind: exact`, `evidence_grade: a`, and carry the printed
row line in `verbatim`. The builder is `scripts/g2_build.py`; it reads the
pdftotext copies in `pdf/` and writes `sources/` and `candidate_rows.json`.
`scripts/g2_sources.py` writes `sources.json`.

| Source | Rows | What |
|---|---|---|
| Onsrud Hard Wood, Soft Wood, MDF, Hard Plywood, Soft Plywood | 5 × 11 | 37-00/37-20 (1), 37-50 (3), 37-60 (4), 37-80 (3) per sheet |
| Onsrud Laminated Chipboard (page 119, upper table) | 10 | same series; 37-60 has 3 cells |
| Onsrud Laminated Plywood (page 119, lower table) | 11 | same series |
| Amana PCD ball nose v2 | 4 | MDF and uncoated chipboard, 6.35 and 9.53 mm |
| Amana Spektra 3D carving v6 | 6 | ball nose in MDF, 0.79 to 6.35 mm, 2/3/4 flutes |
| Amana ball nose v7 | 15 | softwood, hardwood, MDF at 1/16, 3/8, 1/2, 5/8, 3/4 in (the sizes the LUT does not hold) |

Decisions that the reconciler must check (each row's `notes` repeats them):

- **Operation family.** No chart names an operation. V-bit rows use
  `trace`/`finish` (the convention of the existing `chamfer_vbit` rows).
  Ball-nose rows use `pocket`/`roughing` (the convention of the printed
  `amana-ball-*-v7` rows; every chart here states a 1 x D depth rule). One
  row per printed cell. I did not fan rows out to other families.
- **Hardness.** No chart prints a hardness. `hardness_value` is the engine
  proxy per family (hardwood 1450, softwood 600, MDF 1100, hardwood plywood
  1000, softwood plywood 600, particleboard 750), the same convention as
  `onsrud_tapered_ball.json`. See §6 for the table disagreement.
- **Material mapping.** Laminated Chipboard maps to `particleboard`
  (coated). Laminated Plywood maps to `plywood_hardwood` as a best fit; the
  LUT has no laminated family. PCD "Chipboard Without Coating" maps to
  `particleboard`. Spektra "Wood, MDF, Sign-Foam" gives an `mdf` row only;
  "Wood" has no LUT family. PCD "Wood" is not transcribed for the same
  reason.

## 3. The Onsrud 37-series: traps in the transcription

The column of each value comes from its character position under the header
line (`scripts/g2_column_map.py`, offsets 0 to 2 characters). I rendered the
MDF sheet and catalog page 119 (both laminated tables) and read the cells on
the image. They agree with the text. The four other wood sheets have the same
layout and the same 37-series cells, so I did not render them.

Catalog definitions (PCT-19, pages 20 to 22, stored excerpt):

| Series | Type | Flutes | Angle | Cutting diameters in the catalog |
|---|---|---|---|---|
| 37-00 / 37-20 | solid carbide engraving | 1 | 60° / 30° | tip 0.005 to 0.040 in; shank 1/4 in |
| 37-50 | solid carbide V bottom | 2 | 90° | 3/16, 1/4, 3/8 in |
| 37-60 | carbide tipped V bottom | 2 | 90° | 1/2, 3/4, 1 in |
| 37-80 | carbide tipped lettering | 2 | 60° (1 in), 90° (1-1/2 in), 120° and 140° (2 in) | 1, 1-1/2, 2 in |

1. **37-00/37-20: the column is the shank.** The sheets print .004-.006
   under the 1/4 in column, and every part in the series has a 1/4 in shank
   and a tip of 0.13 to 1.0 mm. The rows leave out `diameter_mm`,
   `tip_diameter_mm` and `included_angle_deg`. This is the same frame
   problem as INVENTORY §4 (tip against engaged diameter).
2. **37-60 at 3/8 in.** Every sheet prints a 37-60 cell at 3/8 in. The
   catalog has no 3/8 in 37-60 part. Transcribed as printed.
3. **Laminated Chipboard 37-60.** The rendered page shows .004-.006 under
   3/8 and 9/16 in and .006-.008 under 7/8 in. No 37-60 part has these
   sizes, and the other tables print 3/8, 1/2, 3/4 and 1 in. This is
   probably a layout error in the source. I did not correct it.
4. **37-80 sizes.** The five wood sheets print 1, 1 1/4* and 2** in; the
   laminated tables print 1, 1 1/2 and 2 in. No catalog part is 1-1/4 in,
   and two parts are 2 in (120° and 140°). `included_angle_deg` is set only
   where the catalog has one part at that size (1 in 60°, 1-1/2 in 90°).
   The footnotes "* = 16,000 RPM" and "** = 15,000 RPM" go to
   `rpm_nominal`.
5. **Cut column.** The sheets print "1/2 x D" or "1/2 CED" for 37-50/37-60
   and "Varies" for the others. The exact text is in `ap_rule`. The
   37-50/37-60 rows also carry `ap_max_factor: 0.5` (the printed half D).
6. **The two laminated PDFs are one page.** Catalog page 119 holds both
   tables. The two URLs serve different crops with different sha256, and the
   same text layer.

## 4. What the charts say about the category axis (observations, not ratios)

The direction of MDF against solid wood disagrees between vendors:

- **Onsrud V-bit and tapered ball: no difference.** The 37-series cells are
  identical on all seven wood tables (hard wood, soft wood, MDF, hard
  plywood, soft plywood, laminated chipboard, laminated plywood). The 77-100
  tapered ball is identical on the four sheets that print it. The flat
  series on the same sheets do vary by sheet. Example, 60-100MW at 1/4 in:
  hard wood .014-.016, soft wood .018-.020, MDF .013-.015, hard plywood
  .014-.016, soft plywood .017-.019, laminated chipboard .017-.019.
- **Amana ball nose v7: MDF at or above hardwood.** MDF equals softwood at
  1/16 and 1/8 in, and sits one step (.001 in) below softwood from 1/4 in up.
- **Amana Spektra 3D carving and ZrN 3D profiling: no difference.** One row
  for "Wood, MDF, Sign-Foam". The two charts print the same values for the
  matching tools, so Spektra 3D is not an independent witness. The LUT
  emits the ZrN row as both `mdf` and `softwood` rows; this fetch emits
  `mdf` rows only. The reconciler decides one rule for both.
- **Amana PCD ball nose: MDF below wood.** 1/4 in: MDF .0016-.005, Wood
  .002-.007. 3/8 in: MDF .005-.008, Wood .006-.01. Uncoated chipboard equals
  Wood.
- **Leitz (industrial sizing, grooving and jointing cutters): MDF below
  chipboard, chipboard above softwood.** The feed diagrams print
  "Correction factor for vf". Base plastic-coated chipboard: "MDF = 0.8" in
  most diagrams, "MDF = 0.9" for jointing, "MDF = 0.6" in one diagram
  (pdftotext page 58), "Uncoated chipboard = 1.1", "Solid wood = 0.8"
  (grooving). Base softwood: "Hardwood = 0.8; Chipboard = 1.3; Glulam =
  0.9", "Hardwood = 0.8; Chipboard = 1.2", "Hardwood = 0.9; ... Chipboard =
  1.1", "Hardwood = 0.7; Glulam = 0.8". Leitz prints **no plywood factor**.
  Derived (my inference): at a fixed speed n and tooth count Z, a vf factor
  is an fz factor.
- **Freud (generic solid carbide bits): MDF/Particle Board is the highest
  wood column** at 1/4 in and up (.013-.017 at 1/4 in against hardwood
  .008-.011, softwood .010-.012, plywood .006-.009). Plywood is the lowest
  wood column at 1/4 in, between hardwood and softwood at 3/8 in, and equal
  to hardwood at 1/2 in (.018-.021).
- **Sorotec (reseller, tool type not stated): MDF 2x to 6x wood.** An
  outlier against every other source.
- **Machine builders group categories.** Techno CNC: "Softwood & Plywood",
  "MDF & Particle Board". Sienci: "Softwood/Soft Plywood/MDF" and
  "Hardwood / Hard Plywood".

So no single MDF : wood ratio holds across tool families. For plywood, the
only printed evidence is (a) Onsrud and Freud rows, and (b) the builders'
rule "plywood goes with the solid wood of its face class". No vendor states a
plywood factor for a ball nose or a V-bit.

## 5. Target 3: flat-end series already held in all five categories

Keyed by vendor, subfamily, diameter and flute count (script in the session;
the LUT only):

| Vendor | Subfamily | Diameter mm | Flutes | Printed categories |
|---|---|---|---|---|
| amana | spektra_spiral_plunge | 3.175 | 2, 3 | MDF exact/a; the 4 others derived/b from one Wood/Plywood column |
| amana | spektra_spiral_plunge | 6.0 | 2, 3 | same |
| amana | spektra_spiral_plunge | 6.35 | 2, 3 | same |
| onsrud | 60_000hh_series | 9.525 | 2 | all 5 exact (plywood a, the others b) |
| onsrud | 60_000lh_series | 9.525, 12.7, 15.875 | 2 | all 5 exact |
| onsrud | 60_100mw_compression_spiral | 4.7625 | 2 | all 5 exact |
| onsrud | 60_800_series | 9.525 | 2 | all 5 exact |

No fetch was needed for these. See `lut_discrepancies.md` §3 for the Spektra
caveat. The Onsrud sheets print more flat series in all five categories than
the LUT holds (for example 52-200, 57-200, 60-100C, 60-300); that is a
transcription gap for the flat family, not a G2 gap.

## 6. Janka proxies

The engine has two tables that disagree (INVENTORY §2): `effective_janka_lbf`
(MDF 1100, HDF 1300, particleboard 750, softwood plywood 600, Baltic birch
1200, hardwood-faced plywood 1000) and `family_default_janka` (MDF 700, HDF
900, particleboard 600, softwood plywood 550, hardwood plywood 1100). Neither
table cites a source in the code comments.

What I found:

- **Particleboard.** ASTM D1037 Section 17 defines a hardness test for
  panels (a steel ball test, "modified Janka"). ANSI A208.1-2016 Table B
  prints "Hardness 2225 (500)" N (lb) for grades PBU, D-2, D-3 and M-3; its
  Table A (industrial grades) prints no hardness in this edition. The 1999
  Wood Handbook Table 10-8 (from NPA 1993) prints 2,225 N for M-1, M-S, M-2,
  M-3 and H-1, 4,450 N for H-2 and 6,675 N for H-3. These are **minimum
  requirements**, not typical values. ANSI A208.1 prints the lb value
  itself ("2225 (500)"); for the Wood Handbook N values, 2,225 N = 500 lbf
  is my conversion.
  So 500 lbf is a sourced floor for M-grade particleboard. Neither 600 nor
  750 has a source.
- **MDF.** Wood Handbook Table 10-10 (ANSI A208.2, NPA 1994) has no hardness
  column. The ANSI A208.2-2016 text (codes.iccsafe.org) returned HTTP 403.
  The Weyerhaeuser MDF specification prints density (788 kg/m3) and no
  hardness. The Roseburg data sheet URL returned an HTML page, not a PDF. One
  search hit (an oak-fibre MDF paper) reports hardness in N/mm2, which is not
  a Janka force. **Not published anywhere I could reach.** Neither 700 nor
  1100 has a source.
- **Plywood.** No standard defines a plywood Janka value. The only
  defensible proxy is the face species. The engine's own species table has
  Birch 1260; the proxies 1000, 1100 and 1200 have no source.

## 7. What I searched

Web searches (all 2026-09-24):

- "Onsrud 37-50 series V bottom router bit 90 degree flutes cutting diameter"
  → CNC Router Store, Rennie Tool, the PCT-19 catalog PDF (used).
- "ball nose router bit chip load chart plywood MDF softwood hardwood pdf"
  → Amana 2015 ball chart (toolstoday), Amana spoilboard chart, Freud,
  ShopBot, calculators (toolgrit, workshopcalc, gdptooling: not used,
  calculators are not charts).
- "CNC router chipload chart plywood MDF hardwood softwood ball nose v-bit
  feeds speeds pdf manufacturer" → Freud, Adam's Bits blog, CNCCookbook
  (blog, not used).
- "MDF chip load increase compared to solid wood router bit manufacturer
  recommendation percent" → blogs only (Woodshop News, windridgewoodcrafts,
  cutter-shop, Rennie). **No vendor states an MDF percentage.** The
  "increase chipload 10-20 %" type of rule was not found in any vendor text.
- "Whiteside router bits chip load chart hardwood softwood MDF plywood pdf"
  → Techno CNC chart (used); **no Whiteside chart found**.
- "Leitz lexicon tooth feed fz recommended values ..." → Leitz Lexicon 05
  Routing (used), 11 User encyclopedia (read; its "Favourable fz values"
  table is for circular sawblades, so it is not stored and not used).
- "Vortex Tool ball nose router bit feed speed chart plywood MDF" → forum
  posts only. **No Vortex chart reached.**
- "CMT Orange Tools CNC router bits chip load table ..." → catalogue pages
  only; **no CMT chip-load table found**.
- "Carbide 3D feeds and speeds chart #102 ball nose #301 V-bit ..." →
  community forum only; the docs link is not a PDF chart.
- "Schnittwerte Kugelfräser Holz Sperrholz MDF ..." → Sorotec (used),
  Stepcraft (the URL returns an HTML page, not the PDF), Ingersoll (metal).
- "SpeTool ball nose end mill feeds speeds chart plywood ..." → SpeTool
  pages (both render their tables with JavaScript; the static HTML has no
  table, dead end), Sienci chart (used).
- "\"ball nose\" \"plywood\" chip load per tooth chart pdf ..." → Amana
  ball v6 (used as corroboration), Spektra 3D v6 (used), PCD ball v2 (used).
- Janka: "Janka ball hardness MDF particleboard ASTM D1037 ... ANSI A208.2",
  "Wood Handbook FPL-GTR-282 particleboard MDF ... hardness", "medium density
  fiberboard Janka hardness measured ..." → ANSI A208.1-2016 (used), Wood
  Handbook 1999 ch. 10 mirror (used), Weyerhaeuser spec (used, negative).

Dead ends and URL failures:

- Amana `Spiral-Ball-Nose-Speed-Chart-v8/v9/v10.pdf`: HTTP 404. v7 is the
  current edition.
- `fpl.fs.usda.gov` chapter PDFs (GTR-282 ch. 12, GTR-190 ch. 12) and a
  treesearch download: an HTML block page to curl.
- `codes.iccsafe.org` (ANSI A208.2-2016): HTTP 403.
- Roseburg Arreis Ultra data sheet: HTML, not the PDF.
- Onsrud series web pages 37-00 to 37-80 (`pdf/onsrud_series_37-*.html`):
  the product tables load by JavaScript; the static page gives only the
  cutting-diameter filter (0.1875, 0.25, 0.375 in for 37-50). The PCT-19
  catalog replaced them.

## 8. Files

- `sources.json`: 22 manifest-style entries (pdf_sha256 of the downloaded
  file, stored text path, coverage notes with the row count).
- `sources/<source_id>.txt`: pdftotext -layout text. Excerpts (stated in
  the file header): PCT-19 pages 20-22 and 119; Leitz 05 Routing, only the
  pages with a correction factor.
- `candidate_rows.json`: 101 rows.
- `lut_discrepancies.md`: the missing 37-series, the Freud gaps, the Spektra
  caveat.
- `.stored_index.json`: the builder's intermediate (stored text path, sha
  and row count per source); `g2_sources.py` reads it.
- `pdf/`: the downloads (not committed).
- Scripts: `scripts/g2_html2text.py`, `scripts/g2_column_map.py`,
  `scripts/g2_build.py`, `scripts/g2_sources.py`.
