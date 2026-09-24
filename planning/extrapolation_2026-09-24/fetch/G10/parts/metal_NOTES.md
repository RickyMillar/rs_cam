# G10 fetch, part `metal`: notes

Scope: Harvey Tool and Helical Solutions, GARR TOOL, Kennametal. Extras:
KYOCERA SGS, IMCO Carbide, Sandvik Coromant. Accessed on 2026-09-25.
The files are `metal_sources.json` (12 stored sources, 8 dead ends) and
`metal_statements.json` (53 statements). The script
`scripts/g10_metal_build.py` writes both files and computes the hashes.

All statements have grade c for wood. No document in this part names wood
in an entry rule. Each statement records the printed material in
`material.printed`.

## 1. Result per target

| Target | Result | Best statements |
|---|---|---|
| Maximum ramp angle, general | found (starting ranges) | Harvey/Helical: soft/non-ferrous 3-10 deg, hard/ferrous 1-3 deg |
| Max ramp angle against centre cutting | found | SGS matrix: non-centre-cutting 1-7 deg; centre-cutting 1-90 deg. Helical 2016 p. 6: the angle of descent varies with the end style |
| Max ramp angle against flute count | partly | SGS: 7 flute H-Carb 1 deg, multi-flute 1 deg, 2-3 flute S-Carb 90 deg. IMCO: 7-13 flute 0.5 deg. No vendor prints a formula |
| Max ramp angle against diameter | not published | No vendor prints a split by diameter. The Sandvik graph against diameter is an image, and it is for inserts |
| Max ramp angle, ball / corner radius | partly | SGS Turbo-Carb 56B ball: 25 deg. Harvey and Helical: use a corner radius for ramp and helix |
| Max ramp angle, router / soft-material tools | partly | SGS compression router 5 deg, up-cut router 90 deg, down-cut router "–", CCR end-cut 5 deg |
| Helix diameter, minimum | found, ambiguous | Harvey/Helical: "helix diameter >110-120%" of D (bore or path is not defined) |
| Helix diameter, maximum / no core | found | IMCO: bore = 2D - 2r. Sandvik: max hole = 2 x D3, a smaller cutter leaves a core |
| Helix pitch / helix ramp angle | partly | IMCO helical ramp angle per series: 0.5, 1-2.5, 3, 3-5 deg. Sandvik: pitch <= max ap |
| Plunge feed against side feed | found | Garr 0.5; SGS 0.25 x slot feed; Harvey chamfer 0.40-0.50; Harvey pointed engraver 0.5 |
| Ramp feed against side feed | found | SGS: 1.0 x slot feed at 1-2 deg, 0.25 at about 6 deg. IMCO: 1.0 x slot feed (M series); 1.25-1.6 x chart IPT at 0.5 deg (IPT/C). Sandvik linear: 0.75 |
| Wood / MDF / plywood | not published | The Harvey wood charts (SF_809500, SF_809100) print no entry value |
| Plastics / aluminium | partly | Garr plunge rule covers a "Fiberglass, Plastics, G10" row. SGS plunge only in non-ferrous. Harvey plastics and aluminium charts print no entry value |
| Must not plunge or ramp | found | Harvey/Helical: straight plunge needs a centre-cutting tool. SGS Z5: "Do not plunge." SGS down-cut router: "–" (meaning not stated) |

## 2. What each source prints

- `metal_harvey_ramping_success`: The page prints the starting ramp angles
  3-10 deg (soft/non-ferrous) and 1-3 deg (hard/ferrous). It prefers
  circular ramping.
- `metal_harvey_tool_entry`: The page prints a pre-drill of 5-10% over D and
  a helix diameter of >110-120% of D. It prints the same ramp angles. A
  straight plunge needs a centre-cutting tool. Straight and roll-in entry
  use a 50% feed reduction. A vendor reply says that helical entry also
  suits non-ferrous material.
- `metal_helical_guidebook_2016`: Pages 4-7 print the same rules as the two
  blog posts. Page 6 adds that both centre-cutting and non-centre-cutting
  end mills can ramp, but the angle of descent changes with the end style.
- `metal_harvey_sf_18700` (chamfer, pointed and flat end, 2 flutes): "For
  vertical plunging, reduce Chip Loads to 40%-50%".
- `metal_harvey_sf_25000` (pointed engraver, 1 flute): the vertical plunge
  chip load is 50% of the posted value, and ramping is preferred.
- `metal_harvey_sf_809500` (wood, square upcut): no entry value. This is
  evidence of absence.
- `metal_harvey_helical_interp_tv`: a video page. The text prints no
  number.
- `metal_sgs_catalog_2021`: the End Mill Matrix gives a maximum ramp angle
  and a Center Cutting flag for 26 series. The Entry Methods page gives the
  ramp feed against the ramp angle and the plunge feed. The Z5 chart says
  "Do not plunge."
- `metal_imco_helical_ramp`: the helical entry bore is 2D - 2r. The page
  gives the helical ramp angle and feed per series. It also gives the
  one-step rule for 7 flutes or fewer and the 3xD / 3.75xD opening for
  7-13 flutes.
- `metal_garr_technical`: the plunge feed is 50% ("drop feed by
  approximately 50%"). The troubleshooting page says to decrease the ramp
  angle when the edge chips.
- `metal_sandvik_ramping`: the page is for indexable cutters. It prints a
  linear ramp feed of 75%, a maximum hole of 2 x D3, the core and pip rules,
  and a pitch no larger than max ap.
- `metal_kennametal_ramping_blog`: prose only, with no number.

## 3. Dead ends

| URL | HTTP | Result |
|---|---|---|
| https://www1.mscdirect.com/images/solutions/kennametal/millingTechInfoInterpolation.pdf | 403 | blocked |
| https://productivity.com/wp-content/uploads/2022/08/Kennametal-2023-Solid-Carbide-End-Milling-Inch-Master-Catalog-Interactive.pdf | 404 | removed |
| https://productivity.com/wp-content/uploads/2020/07/Kennametal-2018-Vol.2-Solid-Milling-Kennametal-Master-Interactive.pdf | 404 | removed |
| https://www.carbidedepot.com/ken-application-em-harvi-frac.htm | 403 | blocked |
| https://www.kennametal.com/us/en/products/p.4046274.html (and p.5350644, HARVI I TE / III family pages) | 200 | attribute "Ramping: Blank"; no number |
| https://www.kennametal.com/us/en/resources/engineering-calculators/end-milling/helical-interpolation.html | 200 | a calculator; no limit |
| https://www.helicaltool.com/resources/speeds-feeds | 200 | JavaScript list; curl gets no PDF link |
| https://www.lakeshorecarbide.com/speedsfeeds.aspx | 200 | no entry text |
| https://www.garrtool.com/wp-content/uploads/2018/11/TECHNICAL.pdf with agent "Mozilla/5.0" | 404 | the server filters by user agent; a full Chrome agent string gets 200 (stored) |
| https://www.garrtool.com/doc/pdf/CATALOG_USA.pdf with agent "Mozilla/5.0" | 404 | not retried; TECHNICAL.pdf holds the technical section |
| https://www.garrtool.com/knowledge-base/end-mill-chipping/ with agent "Mozilla/5.0" | 406 | a full agent string gets 200; the same text is in TECHNICAL.pdf p. 3 |
| Sandvik ramping page with agent "Mozilla/5.0" | 200 | an 8 kB shell; a full agent string gets the rendered page (stored) |

Kennametal: 6 URLs gave no number. The part stops Kennametal there. The
only Kennametal source is the prose blog.

Harvey probes that print no entry value (not stored): SF_809100 (wood
downcut), SF_827700 (plastics upcut 2 flute), SF_968700 (aluminium
variable helix square), SF_733400 (miniature square), SF_21500 (tapered
ball), SF_61700 (aluminium corner radius), SF_49500 (plastics ball).

## 4. Searches

1. `Harvey Performance "In The Loupe" ramping angle end mill helical interpolation`
2. `Garr Tool ramping helical interpolation guidelines pdf ramp angle`
3. `Kennametal end mill ramping angle helical interpolation minimum bore diameter technical pdf`
4. `Lakeshore Carbide ramping helical interpolation plunge feed reduction`
5. `garrtool.com technical information ramp angle plunge feed end mill catalog`
6. `harveytool.com speeds feeds pdf "ramp" "plunge" end mill aluminum plastics`
7. `SGS Kyocera end mill "ramping" "helical interpolation" minimum hole diameter maximum ramp angle table pdf`
8. `lakeshorecarbide.com ramping "ramp angle" end mill aluminum recommendation`
9. `Helical Solutions speeds and feeds pdf "ramp" angle "helical interpolation" aluminum H45 notes`
10. `OSG end mill "ramping angle" "helical" "minimum hole diameter" technical data pdf`
11. `Kennametal HARVI solid carbide end mill "ramp angle" "helical interpolation" "min" "max" hole diameter pdf catalog`
12. `Kennametal solid end mill application guide "ramping" "reduce feed" "plunge" degrees HARVI ...`
13. `"garrtool" "When plunging into a solid" feed`
14. `end mill manufacturer "helical" entry "hole diameter" "of the cutter diameter" ... Destiny OR OSG OR Iscar OR Lakeshore`
15. `IMCO Carbide M223 M233 M203 end mill aluminum series flutes` (to find the material of the IMCO series; the answer is from a reseller page and is not stored)

The Harvey S&F index (https://www.harveytool.com/resources/speeds-feeds)
gave the chart list. OSG, Iscar and Destiny gave no reachable document in
the budget.

## 5. Points for the verifier

1. **SGS matrix row alignment.** The End Mill Matrix covers two pages. Page
   22 (PDF p. 7) has the series names. Page 23 (PDF p. 8) has the values.
   The text has no shared key. The part maps the rows by order: 26 names
   and 26 value rows. The end styles and the coolant or chipbreaker columns
   agree with the product index where the part can check them (Series 33 R
   only; ZH1 R only; Series 7 S and B; 43CB chipbreaker "Standard"; Slow
   Helix 27 helix 10/12; CCR "Based upon end style"). Rows 2 and 3 (Z1 and
   Z1P) do not agree with the index, so the part uses neither of them. Look
   at the PDF image to confirm the rows used.
2. **SGS material fit is a colour code.** The text does not show which
   material a series suits. `material.printed` says "not printed per row
   (colour-coded)".
3. **The SGS Z5 values conflict.** The matrix says 7 deg. The Z5 chart
   (p. 34) says "ramp up to 5 degrees".
4. **The SGS down-cut router cell is "–".** The document does not say if the
   dash means "do not ramp" or "no data". The statement uses `no_ramp` only
   as a tentative reading.
5. **The meaning of "helix diameter" in Harvey/Helical is not defined.**
   ">110-120% of tool diameter" can be the bore diameter. Then the path
   diameter is only 0.1-0.2 x D, which is a near-plunge. It can also be the
   path diameter. Then the bore is 2.1-2.2 x D, which leaves a core for a
   flat end mill with IMCO's 2D - 2r rule. The statements give both
   readings as derived values. The part does not pick one.
6. **The Helical 2016 PDF has hidden text layers.** Pages 4-5 contain
   earlier draft text under the visible text. One layer says that a corner
   radius tool "will increase tool wear". The visible layer and the blog
   say "decrease". One layer says "must be reduced by at least 50%". The
   visible layer says "is recommended to be reduced". The statements quote
   the visible layer: the last block of each page in the `-raw` section.
   The stored text for this PDF and for the SGS PDF has two sections,
   `pdftotext -layout` and `pdftotext -raw`. The `-layout` output mixes the
   layers.
7. **The Harvey SF_25000 text says "5 0%".** This is a kerning gap in the
   PDF text layer. The value is 50%.
8. **IMCO "IPT or MMPT x 1.6".** The page does not name the chart column
   for the base IPT. Step 2 names the "Peripheral-HEM values". The ramp
   feed is higher than 1.0 x at 0.5 deg. Do not read it as a reduction.
9. **The IMCO material per series is not printed.** The aluminium label for
   M2xx comes from a reseller page. The part did not store that page.
10. **The HTML text headers say 2026-09-24.** The shared script
    `g5_html_to_text.py` writes that date. The real access date is
    2026-09-25, as `metal_sources.json` records. The part did not change
    the shared script.
