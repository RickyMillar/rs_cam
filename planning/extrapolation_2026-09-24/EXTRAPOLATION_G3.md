# G3: Operation family / pass role (a row exists for the tool in another family)

Status: Phase 2 (trend) done 2026-09-24; Phase 3 (fit, witness) not started; nothing lands before the Phase 4 rulings.

Inputs: the LUT at 67e98529 (`crates/rs_cam_core/data/vendor_lut/observations/*.json`,
389 rows), `fetch/G3/verified_rows.json` (54 rows, this reconciler),
`inventory_cells.csv` (Phase 0) and the FM1 matrix
`planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv`.

Scripts (read-only on the LUT):

- `scripts/g3_verified_rows.py` writes `fetch/G3/verified_rows.json`.
- `scripts/trend_g3.py` prints every table in §1 (`python3 scripts/trend_g3.py`).

Every ratio in this file is **derived** by `trend_g3.py`. No vendor prints a
ratio. A value is the band midpoint, or the one value when a row has no
minimum.

## 0. The gap

The Phase 0 inventory puts **126 refused cells** in G3.

| Sub-class | Cells | Tools and operations | Materials |
|---|---|---|---|
| Bull nose, parallel and trace finishes | 48 | DropCutter, RampFinish, RadialFinish, HorizontalFinish (8 each); Trace 8; ProjectCurve 8 | softwood, hardwood, MDF, hardwood plywood (12 each); 3.175 mm 24, 6.0 mm 24 |
| Tapered ball, profile / trace / pencil | 18 | Profile, Trace, Pencil (6 each) | hardwood, MDF, hardwood plywood (6 each). Softwood ships (formula judged BACKED). |
| Tapered ball, waterline / steep-shallow | 16 | Waterline 8, SteepShallow 8 | all four woods (4 each) |
| V-bit adaptive | 16 | Adaptive 8, Adaptive3d 8 | all four woods (4 each); 6.35 mm and 12.7 mm |
| V-bit waterline / steep-shallow | 8 | Waterline 4, SteepShallow 4 | softwood, hardwood (MDF and plywood are G2) |
| Ball nose finishes, never judged | 20 | Scallop, UnifiedFinish, SpiralFinish (MDF and plywood); DropCutter, RampFinish, RadialFinish, HorizontalFinish (plywood) | MDF 6, hardwood plywood 14 |

Counts by axis (`trend_g3.py` §8): BullNose 48, TaperedBallNose 34, VBit 24,
BallNose 20. Hardwood plywood 40, MDF 32, hardwood 30, softwood 24.
3.175 mm 51, 6.0 mm 51, 6.35 mm 12, 12.7 mm 12.

### The current engine rule

The code was read at 67e98529.

1. **Routing.** `lut_query_for` (`feeds/vendor_normalize.rs` L69) maps the
   operation to a LUT family and role. Adaptive3d goes to Pocket. A
   ProjectCurve on a bull nose or a V-bit returns `None`
   (`RecipeRowLookup::RoutingRefused`). That affects the 8 bull ProjectCurve
   cells.
2. **Lookup.** `passes_must_match` (`feeds/vendor_lookup.rs` L652) rejects
   every row in another **operation family**. The **pass role** is only a
   score: +45 for a match, -25 for a mismatch (`score_observation`, L740).
   So a row of the same family and another role can serve today. A row of
   the same tool in another family never serves. That is the G3 gap in code.
3. **Tool fallback.** `tool_family_compatible` (L758) lets a bull nose
   borrow a flat-end row and a tapered ball borrow a ball-nose row. It does
   not help here. The LUT has no flat-end parallel or trace row, and no
   ball-nose contour or trace row.
4. **Refusal.** After `NoRow` or `RoutingRefused`, `support_for_lookup`
   calls `formula_backing` (`feeds/support.rs`). It returns `Clueless`
   with these reasons:

| Reason constant (`feeds/support.rs`) | Cells |
|---|---|
| `BULL_PARALLEL` | 32 |
| `BULL_TRACE` (Trace and ProjectCurve) | 16 |
| `TAPER_ROUGH` (Profile, Trace, Pencil; the text says "Onsrud 77-100 puts it below half") | 18 |
| `TAPER_CONTOUR_FINISH` | 16 |
| `VBIT_ADAPTIVE` (the text also says "the recipe depth passes the end of the cone") | 16 |
| `VBIT_CONTOUR_FINISH` | 8 |
| `UNJUDGED_REASON` (the `_ =>` arm) | 20 |

Note: the Onsrud 77-100 row itself does not serve the 34 tapered cells. The
lookup filters it out by family. The refusal compares the **formula** with
that row.

## 1. The trend

### 1.1 Which tools appear in two or more families or roles

`trend_g3.py` §1 groups rows by tool (source, tool family, subfamily,
diameter, flutes, material, V-bit angle). **61 tools** hold rows in two or
more families or roles: 171 rows, **68 printed cells**. Most rows are
copies of one cell into several families.

| Tool family | Source | Tools | Families per tool | Values across the families |
|---|---|---|---|---|
| bull nose | Amana corner radius (G3 fetch) | 6 (3 woods x 6.35 / 12.7 mm) | 6: adaptive, contour, pocket (roughing); parallel, scallop, trace (finish) | one printed value, copied |
| tapered ball | Onsrud 77-100 (4 sheets) | 8 (4 woods x 3.175 / 6.35 mm) | 4: adaptive, pocket (roughing); parallel, scallop (finish) | one printed value, copied |
| V-bit | IDC Woodcraft (G3 fetch) | 6 (3 bits x 2 assigned woods) | 3: adaptive, pocket (roughing); trace (finish) | one derived value, copied |
| ball nose | Amana ball v7 | 7 | pocket + adaptive (exact), parallel or scallop (derived c) | exact rows copied; finish rows differ |
| tapered ball | Amana ZrN 3D profiling | 2 | parallel/finish + scallop/semi_finish | both derived c |
| flat end | Amana Spektra v24 | 32 | pocket + adaptive (roughing only) | copied |

No flat-end tool in the LUT holds a finish row and a roughing row of the
same subfamily.

### 1.2 Same tool, finish / roughing (`trend_g3.py` §2)

Counted once per tool and role pair.

```
tool_family        pair                   kinds            stats
ball_nose          finish/roughing        derived/exact    n=3 median 0.16 range 0.13-0.18
ball_nose          finish/semi_finish     derived/derived  n=1 median 0.95 range 0.95-0.95
bull_nose          finish/roughing        exact/exact      n=6 median 1.00 range 1.00-1.00
chamfer_vbit       finish/roughing        derived/derived  n=6 median 1.00 range 1.00-1.00
tapered_ball_nose  finish/roughing        exact/exact      n=8 median 1.00 range 1.00-1.00
tapered_ball_nose  finish/semi_finish     derived/derived  n=2 median 0.92 range 0.83-1.00
```

Per material (§2 of the output) the result is the same: every exact/exact
pair is 1.00 in each wood, and every pair that is not 1.00 has a derived row
on one side.

### 1.3 Same tool, operation family inside one role (`trend_g3.py` §3)

```
ball_nose          pocket/adaptive (roughing)       exact/exact      n=6 median 1.00 range 1.00-1.00
bull_nose          contour/adaptive (roughing)      exact/exact      n=6 median 1.00 range 1.00-1.00
bull_nose          pocket/adaptive (roughing)       exact/exact      n=6 median 1.00 range 1.00-1.00
bull_nose          pocket/contour (roughing)        exact/exact      n=6 median 1.00 range 1.00-1.00
bull_nose          scallop/parallel (finish)        exact/exact      n=6 median 1.00 range 1.00-1.00
bull_nose          trace/parallel (finish)          exact/exact      n=6 median 1.00 range 1.00-1.00
bull_nose          trace/scallop (finish)           exact/exact      n=6 median 1.00 range 1.00-1.00
chamfer_vbit       pocket/adaptive (roughing)       derived/derived  n=6 median 1.00 range 1.00-1.00
flat_end           pocket/adaptive (roughing)       derived/derived  n=26 median 1.00 range 0.77-1.00
flat_end           pocket/adaptive (roughing)       exact/exact      n=6 median 1.00 range 1.00-1.00
tapered_ball_nose  pocket/adaptive (roughing)       exact/exact      n=8 median 1.00 range 1.00-1.00
tapered_ball_nose  scallop/parallel (finish)        exact/exact      n=8 median 1.00 range 1.00-1.00
```

The 0.77 in the flat-end derived row comes from repo-authored "upcut"
Spektra rows (grade c), not from a chart.

### 1.4 The repo's own finish factor (`trend_g3.py` §4)

The Amana ball v7 chart prints no role (`fetch/G3/lut_discrepancies.md` §1).
The LUT holds finish rows beside the printed rows. They are derived, grade c.

```
material  fin_d  finish_row (band, kind/grade)            rough_d rough band      ratio_mid
hardwood  3.175  amana-ball-hardwood-parallel-3 0.0120-0.0240 d/c  3.175   0.0762-0.1270 0.18
hardwood  6.0    amana-ball-hardwood-parallel-6 0.0220-0.0400 d/c  6.35    0.1270-0.1778 0.20
hardwood  6.0    amana-ball-hardwood-scallop-60 0.0250-0.0400 d/c  6.35    0.1270-0.1778 0.21
mdf       3.175  amana-ball-mdf-parallel-3175-2 0.0150-0.0260 d/c  3.175   0.1270-0.1778 0.13
mdf       6.0    amana-ball-mdf-parallel-6000-2 0.0250-0.0420 d/c  6.35    0.1524-0.2032 0.19
softwood  3.175  amana-ball-softwood-parallel-3 0.0180-0.0300 d/c  3.175   0.1270-0.1778 0.16
softwood  6.0    amana-ball-softwood-parallel-6 0.0300-0.0500 d/c  6.35    0.1778-0.2286 0.20
repo factor finish/rough (ball v7): n=7 median 0.19 range 0.13-0.21
```

This factor has no source. These 7 rows serve **42 shipping ball-nose cells**
today (matrix `lut_observation_id`). The ZrN tapered finish and semi_finish
rows (all derived c) serve no cell; the Onsrud 77-100 rows win the lookup.

### 1.5 Different tools on one sheet (`trend_g3.py` §5)

This is the only printed finish-against-roughing contrast in wood. The two
series are **different tools** with different flute forms (Onsrud names
60-200 a low-helix finisher and 60-000 / 60-800 roughers). It is **not** a
per-tool role ratio. Do not fit a k to it.

```
  flat_end  finish 60_100c_compression_spiral / roughing 60_000hh_series  n=3 median 1.11 range 1.00-1.28
  flat_end  finish 60_100c_compression_spiral / roughing 60_000lh_series  n=3 median 1.38 range 1.33-1.53
  flat_end  finish 60_100c_compression_spiral / roughing 60_800_series    n=3 median 1.11 range 1.00-1.28
  flat_end  finish 60_100mw_compression_spira / roughing 60_000hh_series  n=6 median 1.01 range 0.83-1.11
  flat_end  finish 60_100mw_compression_spira / roughing 60_000lh_series  n=7 median 1.21 range 1.12-1.33
  flat_end  finish 60_100mw_compression_spira / roughing 60_800_series    n=4 median 0.94 range 0.83-1.11
  flat_end  finish 60_200_downcut             / roughing 60_000hh_series  n=4 median 0.42 range 0.33-0.44
  flat_end  finish 60_200_downcut             / roughing 60_000lh_series  n=8 median 0.50 range 0.41-0.53
  flat_end  finish 60_200_downcut             / roughing 60_800_series    n=3 median 0.39 range 0.33-0.39
all cross-tool finish/roughing pairs: n=41 median 0.95 range 0.33-1.53
```

The "about 0.4" in `FETCH_NOTES.md` is the 60-200 downcut finisher only. The
compression finishers print the same as the roughers or more.

### 1.6 Bull nose: every row, and the flat-end prediction (`trend_g3.py` §6)

Every bull-nose wood row:

| Origin | Row | Material | D (mm) | Band (mm/tooth) | Kind / grade |
|---|---|---|---|---|---|
| G3 fetch | Amana 46460 | softwood | 6.35 | 0.1778-0.2286 | exact / a |
| G3 fetch | Amana 46460 | hardwood | 6.35 | 0.1270-0.1778 | exact / a |
| G3 fetch | Amana 46460 | MDF | 6.35 | 0.1524-0.2032 | exact / a |
| G3 fetch | Amana 46462 | softwood | 12.7 | 0.2286-0.2794 | exact / a |
| G3 fetch | Amana 46462 | hardwood | 12.7 | 0.1778-0.2286 | exact / a |
| G3 fetch | Amana 46462 | MDF | 12.7 | 0.2032-0.2540 | exact / a |
| LUT | `onsrud-bull-softwood-adaptive-6000-2f` | softwood | 6.0 | 0.055-0.095 | derived / c |
| LUT | `onsrud-bull-hardwood-adaptive-6000-2f` | hardwood | 6.0 | 0.032-0.060 | derived / c |
| LUT | `onsrud-bull-plywood-hardwood-pocket-6000-2f` | hardwood plywood | 6.0 | 0.035-0.065 | derived / c |

The printed cells against Amana flat-end rows of the same material and a
diameter within 0.35 mm (2 flutes):

```
bull_d mat       bull_band       flat source / subfamily                 flat_d flat_band      kind  mid/value  max/max
6.35   hardwood  0.1270-0.1778   amana_compression_spir/compression       6.35   0.0787-0.0787  e/a       1.94     2.26
6.35   hardwood  0.1270-0.1778   amana_spektra_spiral_p/spektra_spiral_p  6.35   -0.1270        d/b       1.20     1.40
6.35   mdf       0.1524-0.2032   amana_compression_spir/compression       6.35   0.1549-0.1549  e/a       1.15     1.31
6.35   mdf       0.1524-0.2032   amana_spektra_spiral_p/spektra_spiral_p  6.35   -0.1524        e/a       1.17     1.33
6.35   softwood  0.1778-0.2286   amana_spektra_spiral_p/spektra_spiral_p  6.35   -0.1270        d/b       1.60     1.80
6.35   softwood  0.1778-0.2286   amana_zrn_3d_profiling/zrn_2d3d_carving  6.35   0.1524-0.2032  e/a       1.14     1.12
12.7   hardwood  0.1778-0.2286   amana_compression_spir/compression       12.7   0.1956-0.1956  e/a       1.04     1.17
12.7   mdf       0.2032-0.2540   amana_compression_spir/compression       12.7   0.2819-0.2819  e/a       0.81     0.90
12.7   mdf       0.2032-0.2540   amana_spektra_spiral_p/spektra_spiral_p  12.7   0.2438-0.2438  e/a       0.94     1.04
12.7   softwood  0.2286-0.2794   amana_spektra_spiral_p/spektra_spiral_p  12.7   0.1448-0.1448  e/a       1.75     1.93
bull/flat mid ratio, flat row derived/b, d=6.35: n=4 median 1.40 range 1.20-1.60
bull/flat mid ratio, flat row derived/c, d=6.35: n=5 median 3.01 range 2.32-4.18
bull/flat mid ratio, flat row exact/a, d=6.35: n=6 median 1.16 range 1.00-1.94
bull/flat mid ratio, flat row exact/a, d=12.7: n=4 median 0.99 range 0.81-1.75
```

(The script also prints the 6.0 mm flat rows. The table above keeps the
same-diameter rows. The `derived/b` counts include the 6.0 mm Spektra rows,
which have the same value.)

Other bull comparisons:

- Amana ball v7 at 6.35 mm: ratio **1.00** in all three woods. The ball v7
  chart also prints the same bands at 12.7 mm (verifier check); the LUT
  holds no 12.7 mm ball v7 row.
- The derived `onsrud-bull-*` rows are **0.37** (softwood) and **0.30**
  (hardwood) of the printed band at 6.35 mm. They serve **7 shipping bull
  cells** today (1 + 5 + 1). The plywood row has no printed column to check.

### 1.7 Machine class, not G3 (`trend_g3.py` §7)

The IDC V-bit rows (benchtop, derived c) sit at **0.26-0.34** of the
midpoint of the 5 Amana 6.35 mm V-groove and engraving trace rows. The IDC
ball rows in the LUT (parallel/finish, derived c) sit at **0.31-0.34** of
the Amana ball v7 band. Two things change at once here: the vendor's
machine class and the role label. So these ratios are G9 evidence. They do
not enter any G3 k.

### 1.8 What the data shows

1. **Where a vendor prints a tool once, the LUT copies one value into every
   family.** Every same-tool exact/exact pair is 1.00 (bull n=6, tapered
   ball n=8, ball n=6). This is **not** a measurement that finish equals
   roughing. It shows that the vendors print no role.
2. **The vendors say the same in words.** Amana titles its 3D charts
   "2D/3D Carving" with one value. Onsrud names one series under several
   applications with one row. IDC and PreciseBits change the stepover for
   the clear pass and the final pass, and keep one feed. Four vendors, four
   tool families (flat, ball, tapered ball, V-bit), one structure.
3. **"A finish figure is the rough figure times k" does not hold as a
   vendor fact for k other than 1.** The only same-tool k below 1 is the
   repo's own (0.13-0.21, n=7, no source). Across different tools on one
   Onsrud sheet the ratio runs 0.33-1.53 (median 0.95, n=41). It has no
   constant k.
4. **k = 1 transfers across tool families as a structure.** It is the same
   in bull, tapered ball, V-bit and the Amana 2D/3D ball charts. It is one
   witness (vendor structure). It is not two.
5. **A same-vendor flat-end row predicts the printed bull row within 0.8x to
   2x** for every printed flat row (n=10: median 1.16 at 6.35 mm, 0.99 at
   12.7 mm). The repo-derived flat rows (grade c) miss by 2.3x to 4.2x.
6. **The Amana bull bands equal the Amana ball bands** at 6.35 mm (and at
   12.7 mm on the chart). Amana treats a bull nose and a ball nose of one
   diameter as one chip-load class.

What the data does **not** show:

- It does not show that a chip load is correct for a finishing pass. No
  vendor tests a role. The engine's own depth, stepover and chip-thinning
  rules act on the pass. Phase 3 must check that a family copy does not
  count a geometric effect twice.
- It does not cover the 3.175 mm bull nose (the smallest printed bull size
  is 6.35 mm), bull, ball or V-bit in plywood, or any V-bit 3D finish.
- It does not give a V-bit clearing value on an industrial machine. The
  IDC clear-pass value is benchtop and derived.
- It does not decide the conflict between k = 1 (vendor structure) and the
  repo's k = 0.19 (derived ball finish rows). §3 names it.

## 2. Sources

Hashes are the sha256 of the downloaded PDF or HTML (first 12 hex). The
stored texts are in `fetch/G3/sources/` (the PreciseBits copies are in
`fetch/G5/sources/`).

| Source id | Vendor | What it prints | Grade | URL reachable | Hash | Verifier verdicts |
|---|---|---|---|---|---|---|
| `amana_corner_radius_spiral_plunge_2f` | Amana (Toolstoday attachment) | Bull nose 46460 (1/4 in) and 46462 (1/2 in), 2 flute, 18,000 RPM, 1 x D; soft wood, hard wood, MDF bands. No operation, no role, no plywood. It does not print the corner radius. | a | yes | `a8fb36819675` | 36 confirmed (6 cells x 6 families); 1 metadata defect fixed in `verified_rows.json` |
| `amana_corner_radius_spiral_plunge_2f_zrn` | Amana | 46460-Z at 1/4 in; same wood values. Footnote says "Reduce feed rate". | a | yes | `59545232358e` | 0 rows; confirmation confirmed |
| `amana_corner_radius_spiral_plunge_2f_spektra` | Amana | 46460-K at 1/4 in and 1/2 in; same wood values. | a | yes | `e2905211adb6` | 0 rows; confirmation confirmed |
| `amana_spektra_3d_profiling_v6` | Amana | "2D/3D Carving" chart: one value per tool and material, no 2D/3D split, no role. | a | yes | `84feaace79e1` | 0 rows; claims confirmed |
| `amana_ball_nose_v7` | Amana | Ball nose, one band per diameter and material, no role. Bands equal the bull bands at 1/4 in and 1/2 in. | a | yes | `e851a270a995` (manifest hash) | 0 rows; both claims confirmed |
| `idcwoodcraft_feeds_speeds_pdf` | IDC Woodcraft (Carbide 3D forum mirror) | Benchtop table: one feed, one RPM per V-bit; a clear-pass stepover and a final-pass stepover. No chip load, no wood category. Starter-set table disagrees on the 60 and 90 degree feeds. | c (derived) | yes | `87405efbd746` | 18 confirmed (3 lines x 2 assigned woods x 3 families) |
| `onsrud_oc06_catalog_2006` | Onsrud | Cutting-data sheets: application table (Single Pass, Roughing, Finishing) and one chip-load row per series. No corner-radius series in the **wood** sheets (the aluminium sheet has 66-300 / 66-350). | reference | yes | `ced6b24535b8` | 0 rows; claims confirmed, one wording narrowed |
| `sorotec_schnittwerte` | Sorotec | One fz per diameter (1, 2, 3, 4, 5, 6, 8, 10, 12 mm) for hardwood, softwood, MDF. No tool type, no role. Dead end for G3. | reference | yes | `a7b9cd7bc68b` | 0 rows; claims confirmed |
| `freud_router_bit_feed_and_speed_for_cnc_20170822` | Freud | One band per diameter and material at cut depth = D. No role, no ball, bull or V-bit. | n/a | **not verified** | `ff1a29ea5a66` | not verified |
| `precisebits_tapered_ball_2f` | PreciseBits (G5 copy) | Stepover 8 % of tip (finishing) and 40 % (roughing). Feed from a test cut; no chip load per role. | n/a | **not verified** | `f131e5a712d2` | not verified |
| `precisebits_vtip_2500` | PreciseBits (G5 copy) | V-tip stepover 0.006 in (final cleanup) and 0.010 in (clearance). Feed from a test cut. | n/a | **not verified** | `d8fbce48a11b` | not verified |

Dead ends (`fetch/G3/FETCH_NOTES.md`):

- amanatool.com product HTML pages: HTTP 403 (WebFetch and curl).
- stepcraft-systems.com `SC_Fraesparameter.pdf`: HTTP 403.
- shopbottools.com `FeedsandSpeeds.pdf`: HTTP 403.
- `pdf/fablab_feeds.pdf`: an S3 "NoSuchBucket" error page.
- `pdf/shopbot_feeds.pdf`: an HTML "429 Too Many Requests" page.
- `pdf/onsrud_h_wood2.pdf`: a 2018 image-only Onsrud Hard Wood sheet with
  no recorded URL. Its 77-100 values equal the LUT. Not a source.
- Searches for Whiteside, Carbide 3D and Harvey bull-nose wood charts
  found no chart. The Amana Multi-Helix and Radius-Chamfer charts are metal
  end mills (not fetched).

Not found anywhere reachable:

- A chart that prints two chip loads for one tool in two roles or families
  in wood.
- A bull-nose wood chart from a vendor other than Amana.
- A printed V-bit chip load for clearing, pocketing or adaptive work.
- Any V-bit guidance for 3D finishing (waterline, steep-shallow).
- A tapered-ball value for 2D contour or trace work that differs from the
  3D value.

## 3. For Phase 3

### 3.1 Candidate forms the trend supports

| Form | Evidence | Range it could claim |
|---|---|---|
| **F1. Family copy at k = 1.** A printed row that names no operation serves every family of its tool family. | §1.2, §1.3, §1.8 items 1-2. One witness: the vendor structure (4 vendors, 4 tool families). | Only the printed diameters and materials of the anchor row, plus the G1 size step when G1 lands. Only where the anchor row prints no operation. |
| **F2. Bull nose from the same-vendor flat row.** | §1.6: 0.81-1.94 on exact rows (n=10). | Not supported as a law. The spread is 2.4x, and the printed bull rows now make it unnecessary at 6.35 and 12.7 mm. |
| **F3. Finish = rough x k, k < 1.** | Only the repo's derived rows (0.13-0.21). | None. No vendor number supports it. |

### 3.2 The ruling Phase 4 must take first

The 36 verified bull rows **copy 6 printed cells into 6 families**. A
tapered-ball contour or trace row would copy the Onsrud 77-100 cell the same
way. Both are the F1 act. Two ways to do it:

- **(a) As rows.** Load the 36 copies (and later tapered copies) into the
  LUT. The 48 bull cells become `VendorBacked`. The card does not show that
  the family was transferred. This breaks the R1 record rule.
- **(b) As a claim.** A LUT row carries one `operation_family`, so there are
  two shapes:
  - **(b1)** One row per printed cell, in one roughing family
    (`pocket/roughing`, the home the Onsrud 77-100 precedent uses). The G3
    `Extrapolation` supplies adaptive, contour, parallel, scallop and trace
    with `rule: "vendor prints one value per tool; role changes the stepover
    and depth, not the chip load"`, the source rows and the range. The card
    shows every transfer.
  - **(b2)** The four Onsrud-precedent rows (adaptive, pocket, parallel,
    scallop), and the claim supplies only trace and contour. The 32
    `BULL_PARALLEL` cells then become `VendorBacked` with no transfer record,
    the same defect as (a).

Recommendation: **(b1)**. Apply the same rule to the bull and the tapered
ball. The existing 32 Onsrud 77-100 copies are the (b2) shape; the ruling
decides whether they collapse to one row per cell.

### 3.3 Second witnesses that could exist

- **Simulation.** Run the engine's chip and force samples on the wanaka or
  terrain fixture. Compare a bull finishing pass and a ball finishing pass at
  the F1 value with the same tool's roughing pass. Check that the chip after
  thinning stays inside the printed band and above the rubbing floor. This
  also tests the "counted twice" risk in §1.8.
- **Same-vendor cross-family check.** Amana bull = Amana ball at 6.35 mm and
  12.7 mm (ratio 1.00). It is the same vendor, so it is a consistency check,
  not an independent witness.
- **Physical rule.** The chip thickness a finish pass makes follows from
  the stepover, the depth and the tool geometry. The engine already models
  that (engaged diameter, chip thinning). A literature cell for finishing
  chip load in wood would be a real second witness. None was found.

### 3.4 Recommendation per sub-class

| Sub-class | Cells | Recommendation | Range if it ships | Stays refused |
|---|---|---|---|---|
| Bull nose finishes, softwood / hardwood / MDF, 6.0 mm | 18 | **Per-family claim F1** from the printed Amana bull cells (6.35 mm anchor). The routing also needs a change: ProjectCurve returns `None` in `lut_query_for`. | 6.35-12.7 mm printed; 6.0 mm is 0.94x of the anchor. | - |
| Bull nose finishes, 3.175 mm | 18 of the 24 | Wait for G1 (a 2x step below the smallest printed bull size). | - | until G1 |
| Bull nose finishes, hardwood plywood | 12 | **Refuse** (G2: no plywood column). | - | yes |
| Tapered ball, profile / trace / pencil (hardwood, MDF, plywood) | 18 | **F1, same rule as the bull**, pending §3.2. Anchor: Onsrud 77-100 (prints all four woods incl. hardwood plywood). The tapered lookup keys on the engaged diameter (G5 frame question). | 3.175 and 6.35 mm printed. | - |
| Tapered ball, waterline / steep-shallow | 16 | **F1**, as above. | as above | - |
| V-bit adaptive / Adaptive3d | 16 | **Refuse.** The only evidence is IDC, grade c, benchtop (G9). The refusal also has a geometric cause: the recipe depth passes the end of the cone (G5). Revisit after G5 and G9. The 18 IDC rows stay in `verified_rows.json` as the record; they do not load (grade c, benchtop, G9), so the `idc_vbit` subfamily needs no mapping. | - | yes |
| V-bit waterline / steep-shallow | 8 | **Refuse.** Nothing is printed. | - | yes |
| Ball nose finishes, MDF (Scallop, UnifiedFinish, SpiralFinish) | 6 | **Per-family F1** from the printed Amana ball v7 MDF rows, but only after §3.5 is ruled. | 3.175 and 6.35 mm printed | - |
| Ball nose finishes, hardwood plywood | 14 | **Refuse** (G2 first: no ball plywood row). | - | yes |

Generic or per-family: F1 has one witness (the vendor structure). By PLAN
§3 step 3 it stays **per-family**: bull nose (Amana corner radius), tapered
ball (Onsrud 77-100) and ball nose (Amana v7). It becomes generic only if
the §3.3 simulation witness agrees.

Cells that F1 could move, at most: 18 bull + 34 tapered + 6 ball = **58 of
126**. **68** stay refused: 18 bull at 3.175 mm (G1), 12 bull plywood (G2),
16 V-bit adaptive, 8 V-bit 3D finish, 14 ball plywood (G2). Of those 68,
44 need G1 or G2 first.

### 3.5 Open conflicts Phase 3 must resolve

1. **k = 1 against the repo's k = 0.19.** Seven derived ball finish rows
   (`amana-ball-*-parallel-*`, `amana-ball-hardwood-scallop-6000-2f`) serve
   **42 shipping cells** at 0.13-0.21 of the printed band. No source prints
   that factor. F1 says the value is the printed band. Phase 3 must choose
   one of these. The simulation witness in §3.3 is the test. This is the
   biggest open question in G3.
2. **The derived `onsrud-bull-*` rows.** They sit at 0.30-0.37 of the
   printed Amana band and serve 7 shipping bull cells. Recommendation:
   supersede the softwood and hardwood rows with the printed cells. The
   plywood row has no printed check (G2). Cross-group: the other 6.0 mm bull
   roughing cells ship today on the flat-end fallback, mostly the bandless
   Spektra rows (1.17-1.60x under the printed bull band, §1.6). A printed
   bull roughing row supersedes them and gives them a band. That closes part
   of G4's 31 bandless BullNose cells. G3 and G4 Phase 3 must agree which
   group owns that change.
3. **`missing_project_curve_rows`** (`feeds/vendor_normalize.rs` L106) says
   the LUT has bull-nose "adaptive, pocket and scallop rows". The LUT has no
   bull scallop row. The text needs a correction when G3 lands.
4. **Verified-row metadata.** The corner radius (1/16 in, 1/8 in) and
   "up-cut" come from unstored Toolstoday product pages.
   `verified_rows.json` marks them derived. A loader must not treat them as
   printed.

## 5. The landing (A3, 2026-09-24)

Ruling A3 (b1): one row per printed cell; a visible G3 family claim serves
the other operation families. Plan and decisions: `A3_PLAN.md`.

| Commit | Step |
|---|---|
| ee14e824 | `extrapolation/family.rs`, `FeedsSupport::FamilyTransferred`, the Onsrud 77-100 tapered rule; TAPER_* judgements deleted |
| 5ca7fedf | one Onsrud 77-100 row per printed cell (24 copies deleted; no number moves) |
| (step 4) | the Amana corner-radius bull rows, the bull rule, bull ProjectCurve routed, the plywood-only bull text, the bull soft/hard cap 1.33, the nearer-diameter tie-break |

Found in step 4: the lookup's diameter term saturates at 2x, so for a
3.175 mm bull nose the 6.35 mm row (0.5x) and the 12.7 mm row (0.25x)
tied and the id picked 12.7; the size law then refused. Fix: on an equal
score the row nearer in diameter wins before the id (V-bits keep the id
tie-break until B4). No other matrix cell moved.

### Cells that moved (FM1)

- Refusals 496 (before A3) -> 426: 34 tapered cells and 36 bull cells ship.
- 6 softwood tapered Profile / Trace / Pencil cells: formula -> the printed
  Onsrud band.
- 36 shipping bull cells change row, from a flat end-mill stand-in or the
  deleted repo-derived onsrud-bull rows to the bull's own printed Amana
  corner-radius row:

| D mm | Operation | Material | Old row, chip | New row, chip | Ratio |
|---|---|---|---|---|---|
| 3.1750 | Profile | hardwood | amana-flat-hardwood-contour-3175-2f 0.0227 | amana-bull-hardwood-pocket-6350-2f-cr 0.0999 | x4.40 |
| 3.1750 | Profile | mdf | onsrud-mdf-60-100mw-1_8-finish 0.2794 | amana-bull-mdf-pocket-6350-2f-cr 0.1165 | x0.42 |
| 3.1750 | Profile | softwood | onsrud-softwood-60-100mw-1_8-finish 0.3048 | amana-bull-softwood-pocket-6350-2f-cr 0.1331 | x0.44 |
| 3.1750 | SteepShallow | hardwood | amana-flat-hardwood-contour-3175-2f 0.0227 | amana-bull-hardwood-pocket-6350-2f-cr 0.0999 | x4.40 |
| 3.1750 | SteepShallow | mdf | onsrud-mdf-60-100mw-1_8-finish 0.2794 | amana-bull-mdf-pocket-6350-2f-cr 0.1165 | x0.42 |
| 3.1750 | SteepShallow | softwood | onsrud-softwood-60-100mw-1_8-finish 0.3048 | amana-bull-softwood-pocket-6350-2f-cr 0.1331 | x0.44 |
| 3.1750 | Waterline | hardwood | amana-flat-hardwood-contour-3175-2f 0.0227 | amana-bull-hardwood-pocket-6350-2f-cr 0.0999 | x4.40 |
| 3.1750 | Waterline | mdf | onsrud-mdf-60-100mw-1_8-finish 0.2794 | amana-bull-mdf-pocket-6350-2f-cr 0.1165 | x0.42 |
| 3.1750 | Waterline | softwood | onsrud-softwood-60-100mw-1_8-finish 0.3048 | amana-bull-softwood-pocket-6350-2f-cr 0.1331 | x0.44 |
| 6.0000 | Adaptive | hardwood | onsrud-bull-hardwood-adaptive-6000-2f 0.0460 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x3.20 |
| 6.0000 | Adaptive | mdf | amana-flat-mdf-adaptive-6000-2f-spektra 0.1524 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x1.13 |
| 6.0000 | Adaptive | softwood | onsrud-bull-softwood-adaptive-6000-2f 0.0750 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x2.62 |
| 6.0000 | Adaptive3d | hardwood | amana-flat-hardwood-pocket-6000-2f-spektra 0.1270 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x1.16 |
| 6.0000 | Adaptive3d | mdf | amana-flat-mdf-pocket-6000-2f-spektra 0.1524 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x1.13 |
| 6.0000 | Adaptive3d | softwood | amana-zrn-flat-softwood-pocket-6000-2f 0.2032 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x0.97 |
| 6.0000 | Face | hardwood | amana-flat-hardwood-pocket-6000-2f-spektra 0.1270 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x1.16 |
| 6.0000 | Face | mdf | amana-flat-mdf-pocket-6000-2f-spektra 0.1524 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x1.13 |
| 6.0000 | Face | softwood | amana-zrn-flat-softwood-pocket-6000-2f 0.2032 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x0.97 |
| 6.0000 | Pocket | hardwood | amana-flat-hardwood-pocket-6000-2f-spektra 0.1270 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x1.16 |
| 6.0000 | Pocket | mdf | amana-flat-mdf-pocket-6000-2f-spektra 0.1524 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x1.13 |
| 6.0000 | Pocket | softwood | amana-zrn-flat-softwood-pocket-6000-2f 0.2032 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x0.97 |
| 6.0000 | Profile | hardwood | onsrud-hardwood-60-100mw-1_4-finish 0.3704 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x0.40 |
| 6.0000 | Profile | mdf | onsrud-mdf-60-100mw-1_4-finish 0.3391 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x0.51 |
| 6.0000 | Profile | softwood | onsrud-softwood-60-200-1_4-finish 0.1492 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x1.32 |
| 6.0000 | Rest | hardwood | amana-flat-hardwood-pocket-6000-2f-spektra 0.1270 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x1.16 |
| 6.0000 | Rest | mdf | amana-flat-mdf-pocket-6000-2f-spektra 0.1524 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x1.13 |
| 6.0000 | Rest | softwood | amana-zrn-flat-softwood-pocket-6000-2f 0.2032 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x0.97 |
| 6.0000 | SteepShallow | hardwood | onsrud-hardwood-60-100mw-1_4-finish 0.3704 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x0.40 |
| 6.0000 | SteepShallow | mdf | onsrud-mdf-60-100mw-1_4-finish 0.3391 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x0.51 |
| 6.0000 | SteepShallow | softwood | onsrud-softwood-60-200-1_4-finish 0.1492 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x1.32 |
| 6.0000 | Waterline | hardwood | onsrud-hardwood-60-100mw-1_4-finish 0.3704 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x0.40 |
| 6.0000 | Waterline | mdf | onsrud-mdf-60-100mw-1_4-finish 0.3391 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x0.51 |
| 6.0000 | Waterline | softwood | onsrud-softwood-60-200-1_4-finish 0.1492 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x1.32 |
| 6.0000 | Zigzag | hardwood | amana-flat-hardwood-pocket-6000-2f-spektra 0.1270 | amana-bull-hardwood-pocket-6350-2f-cr 0.1472 | x1.16 |
| 6.0000 | Zigzag | mdf | amana-flat-mdf-pocket-6000-2f-spektra 0.1524 | amana-bull-mdf-pocket-6350-2f-cr 0.1718 | x1.13 |
| 6.0000 | Zigzag | softwood | amana-zrn-flat-softwood-pocket-6000-2f 0.2032 | amana-bull-softwood-pocket-6350-2f-cr 0.1963 | x0.97 |

The large moves are the flat finish rows (Onsrud 60-100mw finish, 0.28-0.37
mm/tooth on a 6 mm tool) going to the bull row (x0.40-0.51), and the
repo-derived onsrud-bull adaptive rows (0.30-0.37 of the printed band)
going to the printed band (x2.6-3.2).

### Open for the operator

- The four bull literature cells (`bull_6mm_adaptive2d_oak`,
  `bull_6mm_pocket_oak`, `bull_12mm_pocket_oak`,
  `bull_6mm_scallop_oak_unadvised`) band the bull on FLAT-END charts
  (their feed-per-tooth bands copy the flat twin cells). The printed Amana
  corner-radius chart puts the bull at about 0.13-0.18 mm/tooth at 6 mm,
  above those bands. Proposal: add the Amana chart to sources.toml and
  re-band the cells from it, or keep the flat proxy and accept a HIGH
  verdict. `bull_12mm_pocket_oak` reads its pocket envelope Outside.
- B3 still owns the 42 ball-nose finish cells and the 6 ball-nose MDF cells.
