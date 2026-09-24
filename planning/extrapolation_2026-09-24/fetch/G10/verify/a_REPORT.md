# G10 verification, parts `wood` and `metal` (verifier a)

Date: 2026-09-25. Scope: `parts/wood_*.json`, `parts/metal_*.json` and the
two `*_NOTES.md` files. The machine-readable result is `a_verdicts.json`.
Re-downloaded bytes and scratch copies are in `a_work/`.

## 1. Counts

Statements: 100 (wood 47, metal 53). I checked all of them. None is skipped.

| Verdict | Count |
|---|---|
| confirmed | 96 |
| wrong | 3 |
| grade_wrong | 1 |
| not_found | 0 |

Sources: 39 records (29 with bytes, 10 dead ends without bytes).

| Rehash | Count |
|---|---|
| match | 21 (all PDFs, plus one ToolsToday HTML page) |
| changed | 8 (HTML pages only; the body text is identical) |
| unreachable | 0 |
| not_checked | 10 (dead ends with no stored bytes) |

## 2. Statements that are not confirmed

| Statement | Verdict | Reason | Correct value |
|---|---|---|---|
| `g10-metal-harvey-sf18700-plunge` | wrong | The chart's Effective Cutter Diameter columns run 0.015 to 0.500 in. No 1.000 in column exists. | `diameter_mm` [0.381, 12.7] |
| `g10-metal-harvey-sf25000-plunge` | wrong | The first material column of SF_25000 is "Plastics: Non-Filled, Glass Filled, Carbon Fiber, G10". The label "chart materials (metals)" / `any_metal` leaves plastics out. | material includes plastics, non-ferrous, cast iron, steels, high temp alloys |
| `g10-metal-harvey-sf25000-ramp-pref` | wrong | The same material error. | as above |
| `g10-wood-vortex-veining-plunge` | grade_wrong | The Series 3700 veining bits print SMALL DIA .094/.125 and RADIUS .125/.250. They groove "while rounding the top edges of the slot". The radius is a top-edge roundover, so `ball_nose` over-claims. | `tool_family` other (veining bit with roundover); grade c unchanged |

The values, units and quotes of these four statements are right.

## 3. Confirmed statements with remarks

- `g10-wood-onsrud-pct19-potted`: the page is p32, not p31.
- `g10-wood-whiteside-upcut`: the text is on PDF p1, not p2.
- `g10-wood-tt-compression-mdf`: the parameter is `plunge_feed`, but the
  derived names say ramp. ToolsToday prints "ramp-down or plunge rate". Its
  260/130 does not agree with the current Amana v8 row (3/8 in 2 flute MDF:
  400/200). The video used an earlier chart edition.
- `g10-wood-idc-v90`: section 2 prints 45/25, and the starter-set table
  prints 50/25. The fetch notes give only the 60 deg conflict.
- `g10-metal-sgs-mx-upcut`, `g10-metal-sgs-mx-downcut`: the matrix prints
  flute count 2. The statements have `flutes` null.
- `g10-metal-imco-flute-steps`: IMCO Method A lists IPT/C 7 for a second
  step. The rule says that seven or fewer flutes need one step.
- `centre_cutting` is inferred, not printed, in these statements:
  `g10-wood-onsrud-guide-pcd`, `g10-wood-idc-spiral-drill`,
  `g10-metal-sgs-plunge-feed`, all `g10-metal-imco-*`,
  `g10-metal-sandvik-linear-75`.

## 4. Flagged points

1. **Amana V-groove v16.** The reading is right. I read all 36 rows. All 24
   one-flute rows from 40 to 110 deg print Ramp Down = Feed/2 in all six
   materials. Only RC-1146 (120 deg) and RC-1101 (150 deg) print Ramp =
   Feed. All 2-flute rows print Feed/2. The footer rule gives Feed/1 for one
   flute, so the column does not follow the rule. RC-1146 and RC-1101 also
   do not agree with their own chip load (14,000 x 1 x .0024 = 33.6 IPM,
   printed 90). They look like copied 2-flute rows.
2. **Amana "Ramp Down".** No Amana document defines it. I searched 33
   Amana text files in `fetch/G*/sources` and the 6 G10 PDFs. "Ramp" occurs
   only as the column header and the rule. "Plunge" occurs only in product
   names. The amanatool.com HTML pages return 403. The Spektra chart v27
   (current) prints the same rows as v24.
3. **IDC plunge column.** All 14 fractions recompute. The asterisk is on
   one cell only: Plunge "60*" of the "Drilling" row, 1/8 in DRILLING
   ENDMILL table, PDF p11. The "Conventional" row (25) has no asterisk.
4. **SGS End Mill Matrix.** PDF p7 and p8 are a two-page spread. Each page
   has 26 rows at the same vertical positions. The order mapping is
   therefore also a position mapping, and each ramp value in the
   statements is right. Z5: the matrix prints 7 deg, and the Z5 chart
   (catalogue p34) prints "ramp up to 5 degrees ... Do not plunge." The
   catalogue gives no reason for the difference. Compression Router 5,
   Up Cut Router 90, Down Cut Router "–" are right. The dash has no
   legend. The part missed two data items in the matrix image:
   - The router, CCR and Slow Helix rows are colour-coded "Non Ferrous N5
     to N7" only.
   - The legend prints "Plunging not recommended in these materials" for
     Stainless M1-M3, High Temp S1-S3, Titanium S4 and Hardened H1-H4.
5. **IMCO.** The formula "(tool diameter x 2) - (corner radius x 2)" is on
   printed p130. The angles are right: IPT/C 7-13 0.5 deg, APT/C 5 3 deg,
   M5xx/M7xx/M8xx/M9xx/E1x/M104 1-2.5 deg, M2xx 3-5 deg.
6. **Harvey / Helical "helix diameter".** The documents do not define it.
   Blog: "use a programmed helix diameter of greater than 110-120% of the
   cutter diameter." Guidebook p4: "We recommend a programmed helix
   diameter >110-120% of tool diameter." Neither says bore or path.
7. **Helical 2016 hidden layers.** I rendered p4 and p5. All quoted numbers
   and the words "decrease tool wear" and "is recommended to be reduced by
   at least 50%" are in the visible layer. The SGS PDF p9 also has several
   text layers. All of them give the same numbers.
8. **Amana 10 deg (Burnette).** Not found in a primary source. The Burnette
   reseller page prints it without a citation.
9. **Vortex 45 deg at 1/3 feed.** Found on
   https://clebitco.com/speeds-and-feeds/ (Cle Bit Co, a reseller and
   sharpening shop). It is not a Vortex document. The sentence follows a
   paragraph about the Vortex #3420 bit. The copy is in
   `a_work/clebitco_speeds_and_feeds.html`. Do not cite it as Vortex.

## 5. Sources

- All PDF sources and the two dead-end PDFs match `raw_sha256`.
- The claimed re-use of earlier groups is right. The same raw sha256 occurs
  in `fetch/G1`–`G8/sources.json` for each source that makes the claim.
- Eight HTML pages changed bytes. For each, `g5_html_to_text.py` on the
  fresh copy gives a body text identical to the stored text:
  `wood_onsrud_plastics_faq2`, `wood_toolstoday_calc_video`,
  `wood_axyz_compression_bit_tip`, `metal_harvey_ramping_success`,
  `metal_harvey_tool_entry`, `metal_harvey_helical_interp_tv`,
  `metal_sandvik_ramping`, `metal_kennametal_ramping_blog`.
- Access caution: Harvey returns 403 to the requested agent string (X11,
  Chrome/124.0). A Windows Chrome/126.0.0.0 string gets 200. Sandvik sends
  an 8 kB shell unless the request has `--compressed` and Accept headers.
- Header date: five text files carry `accessed_on: 2026-09-24`:
  `metal_harvey_ramping_success.txt`, `metal_harvey_tool_entry.txt`,
  `metal_harvey_helical_interp_tv.txt`, `metal_sandvik_ramping.txt`,
  `metal_kennametal_ramping_blog.txt`. All wood text files carry
  2026-09-25. I did not fix them.
