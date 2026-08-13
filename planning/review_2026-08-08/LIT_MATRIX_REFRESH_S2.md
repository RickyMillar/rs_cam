# S-2 — literature-matrix source refresh (DR-URL, DR-WS) + Checkpoint K (e1)

Date: 2026-08-13. Branch `tech-debt-3`. Base `2b1a5c66`.
Procedure: `.claude/skills/refresh-lit-matrix/SKILL.md`.
Ledger rows discharged: **DR-URL**, **DR-WS**
(`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4.4).
Checkpoint ruling executed: **K-(e1)**
(`planning/review_2026-08-08/ORCHESTRATION_LOG.md` §"Checkpoint K",
binding: *"S-2's `/refresh-lit-matrix` re-verifies the cell's sources and
re-baselines if warranted"*).

---

## 1. Method

Every one of the 23 distinct `citation_url` values in
`crates/rs_cam_core/tests/literature_matrix/sources.toml` (32 rows) was
fetched individually on **2026-08-13** with a browser user-agent and
redirects followed, and the HTTP status plus the *content actually
served* was recorded. Content matters independently of status: this run
found a failure class that a status probe cannot see (§3).

Primary documents were retrieved and read, not summarised: the Onsrud
*Hard Wood Cutting Data Recommendations* PDF and the Freud *Router Bit
Feed Rates and Speeds for CNC* PDF were downloaded and text-extracted in
full, and Whiteside's own 2024 Master eCatalog (11.4 MB) and CNC brochure
were downloaded and text-extracted to settle DR-WS by evidence rather
than by re-assertion.

---

## 2. Per-URL disposition — 32 rows, 23 distinct URLs

Measured 2026-08-13. `->` marks a URL that changed.

### A. Did not resolve to a document — 9 rows, 6 distinct URLs, all REPLACED

| key(s) | measured | new `citation_url` |
|---|---|---|
| `toolgrit` | **404** | `www.toolgrit.com/tools/wood-feeds-calc` — publisher live, calculator relocated |
| `dapra_rctf` | **404** (Kennametal) | `www.dapra.com/articles/radial-chip-thinning` — the row is *named* DAPRA and pointed at Kennametal; it now cites its own publisher, and the stored formula is numerically self-verifying against it (§7) |
| `kennametal_metals`, `kennametal_chipload` | **404** | `www.kennametal.com/us/en/resources/engineering-calculators.html` (200). Also **RE-TIERED** gold -> engineering (§4) |
| `cutter_shop` | **NXDOMAIN** *(new)* | `cutter-shop.com/chip-load-chart/` — same publisher (Cutter Shop Limited, England and Wales, co. no. 06862648), domain relocated |
| `shapeoko_wiki` | **connection refused**, 3/3 *(new)* | `shapeokoenthusiasts.gitbook.io/shapeoko-cnc-a-to-z/feeds-and-speeds-basics` — **145 citations across the matrix** |
| `gwizard_hwood`, `gwizard_swood`, `gwizard_plywood` | **soft 404** *(new)* | `www.cnccookbook.com/feeds-speeds-cnc-wood-cutting/` — see §3 |

**Five of these nine rows, across three URLs, are NEW since the
2026-08-04 census** (`cutter_shop`, `shapeoko_wiki`, the three
`gwizard_*`). The four the DR-URL ledger named (`toolgrit` plus the
three rows sharing the dead Kennametal URL) were still dead and are now
repaired.

### B. Resolved, but not to what the row is cited for — 2 rows, RE-TIERED

| key | measured | disposition |
|---|---|---|
| `whiteside_wood` | 200; publisher's own catalogue contains no chart | **RE-TIERED** gold -> engineering (§4) |
| `whiteside_chipload` | 200; publisher publishes no chipload artefact at all | **RE-TIERED** gold -> community (§4) |

### C. Resolved; numbers re-read out of the document — 2 rows

| key | disposition |
|---|---|
| `onsrud_hwood` | **RE-VERIFIED**; full chip-load table re-read (§5). `last_verified` **and** `url_verified` bumped |
| `onsrud_doc_rule` | **RE-VERIFIED** + **name corrected** — the cited PDF does not contain the rule the row's name asserted (§6) |

### D. Resolved; link re-confirmed, numbers NOT re-read — 13 rows

`onsrud_swood`, `onsrud_plywood`, `onsrud_plastic`, `onsrud_drill`,
`onsrud_soft_plastic_cutting_data`, `fpl_wood_handbook`, `shopbot`,
`vectric_default`, `vectric_drill_default`, `amana_plastic_oflute`,
`simona_hdpe_machining`, `sumitomo_kc_table`,
`aluminum_machining_guide` — all HTTP 200. `url_verified` bumped to
2026-08-13; **`last_verified` deliberately left alone**, because the two
clocks answer different questions and nobody re-read these charts.

### E. Undetermined — 6 rows, deliberately NOT advanced

| key(s) | measured |
|---|---|
| `shaw_metal_cutting`, `shaw_chipload` | **202** — OUP bot challenge. 202 is not a document |
| `amana_spektra`, `amana_vbit`, `plastic_machining_guide`, `woodweb` | **403** — bot-block, to curl *and* to a browser-shaped fetcher |

`url_verified` on all six is left absent or unchanged rather than
asserting a check nobody passed. Ledgered **DR-403**.

### F. Added — 1 new row

`freud_router_cnc` — gold, retrieved and read in full (§5.3).

*(9 + 2 + 2 + 13 + 6 = 32 rows, plus 1 added = 33.)*

### Closing measurement — the whole set re-tested after the edits

The full post-refresh URL set was fetched again on 2026-08-13, same
method:

| | before (23 URLs) | after (25 URLs) |
|---|---:|---:|
| HTTP 200 | 14 | **21** |
| hard 404 | 2 | **0** |
| dead host (NXDOMAIN / refused) | 2 | **0** |
| soft 404 (200, article gone) | 1 | **0** |
| 403 / 202 undetermined | 4 | 4 |

The four undetermined are unchanged and unchangeable from here
(`amanatool.com` ×1 URL, `curbellplastics.com`, `woodweb.com` — 403 to
curl *and* to a browser-shaped fetcher; `global.oup.com` — 202). Their
`url_verified` is deliberately not advanced. **Every URL that could be
resolved, resolves.**

---

## 3. New failure class: the soft 404

`https://www.cnccookbook.com/feeds-speeds-wood/` returns **HTTP 200,
zero redirects**, and serves the site's home-page shell. The article is
gone. Neither check the repo owns can see this:

- the offline URL-shape check (`freshness::audit_citation_urls`, P1)
  never performs a GET;
- a status probe — the thing DR-P3 would have added — reads 200 and
  passes.

Only reading the served content finds it. Recorded as its own ledger row
(**DR-SOFT404**) because it changes what "verify a URL" has to mean.

What the live successor actually publishes, verbatim:

```text
Hardwood: 0.0145 IPT
Softwood: 0.0152 IPT
MDF: 0.0131 IPT
Plywood: 0.0143 IPT
```
```text
Note, those are typical numbers for a 1/2" diameter carbide end mill
cutting a full width slot to a depth of 1/4".
```

That is **four single-diameter figures and no diameter table**. The
three `gwizard_*` rows are cited on **12 `feed_per_tooth` bands and 21
`rpm` bands**, none of which this row can support below 1/2 in. Not
re-derived here (out of S-2 scope); ledgered **DR-GWIZ**.

---

## 4. DR-WS — the Whiteside tier question, answered

**The tier rule this refresh applied**, stated so it is auditable:
`gold` means *a retrievable document containing the numbers the row is
cited for*; `engineering` means *a live manufacturer/technical page that
does not publish those numbers* (the existing `aluminum_machining_guide`
/ Sandvik row is precisely this and is already `engineering`);
`community` means *a practice statement, not a cited chart*. No code
reads `authority_tier` — verified by a repo-wide grep (excluding
`target/` and `.git/`), which returns hits only in `sources.toml` itself
and in planning prose — so this is a documentary correction with no
behavioural consequence, exactly as the DR-WS ledger row states.

**Evidence gathered 2026-08-13.** Whiteside's downloads index publishes
six items. Three are documents; all three were downloaded and
text-extracted:

| document | size | `chip load` hits | `RPM` hits |
|---|---:|---:|---:|
| `WM_Full_eCatalog_2024.pdf` (Master Catalog) | 11.4 MB | 1 — *saw blades*, unrelated | **0** |
| `CNC_Brochure_1-14-19.pdf` | 3.7 MB | 0 | **0** |
| `Fine-Woodworking-Article.pdf` | 0.6 MB | 0 | **0** |

The remaining three index entries are Vectric / Carveco / Fusion 360
tool-file downloads, not documents. **Whiteside publishes no feed/speed,
RPM, or chipload chart in any of its own downloadable documents.** The
only retrievable Whiteside cutting guidance is a per-product-page RPM
string (e.g. `Recommended RPM 16,000-18,000` on RD5200CB). This
reproduces the 2026-08-05 finding independently and on stronger
evidence: that one searched the index page and the open web, this one
searched the publisher's own full catalogue.

**Answer, part 1 — tier.** `gold` is unsupported on both rows.

- `whiteside_wood` -> **`engineering`**. It is cited on **14 `rpm`
  bands**, and per-product `Recommended RPM` strings from the
  manufacturer genuinely support an RPM band. Its 1 `axial_doc`, 3
  `radial_woc` and 5 invariant citations are *not* supported by anything
  retrievable, and the row now says so.
- `whiteside_chipload` -> **`community`**, not `engineering`. Unlike
  Kennametal or Sandvik it is not even a technical page with the table
  missing: the sole chipload-bearing artefact is a Vectric `.tool`
  library on a Dropbox share which the publisher's own page disclaims as
  *"a recommended starting point ... speeds and feeds may need to be
  adjusted"*. That is a practice statement.

**Answer, part 2 — do not merge.** The ledger called merging "a
judgement call". The call is **no**, and the reason is the thing the
evidence above establishes: the two rows are *not* two names for one
absent chart. They back different claims with different support —
`whiteside_wood` backs RPM and has retrievable (if weak) support;
`whiteside_chipload` backs feed-per-tooth and has none. Merging them
would erase the only distinction in the pair that carries information,
and would leave 14 supportable RPM citations and 20 unsupportable
feed-per-tooth citations sharing one undifferentiated row.

**Retiring `whiteside_chipload` was considered and not taken.** It is
cited on 20 `feed_per_tooth` bands; **zero** of them cite it as their
sole source (co-citations: `onsrud_hwood` 13, `amana_spektra` 12,
`onsrud_swood`/`gwizard_swood`/`gwizard_hwood` 4 each, `dapra_rctf` 3),
so dropping the key would leave no band uncited and is a cheap follow-up
for whoever wants it. It is kept here because deleting the row would
delete the *record* that 20 bands were once justified by a chart that
does not exist. Ledgered **DR-WSCITE**.

---

## 5. Checkpoint K (e1) — the Ipe cell's three sources

Cell `flat_3mm_pocket_ipe_extreme` (`cells.toml:4957`), band
`[cell.expected.feed_per_tooth] min = 0.013 / max = 0.027`, sources
`["onsrud_hwood", "gwizard_hwood", "fpl_wood_handbook"]`.

### 5.1 Verdict: the literature does **not** support 0.013-0.027, and none of the three cited sources ever did

| cited source | what it actually publishes for Ø1/8 in (3.175 mm) hardwood | supports 0.013-0.027? |
|---|---|---|
| `onsrud_hwood` | Ten series carry a 1/8 in entry; the whole column spans `.002"-.012"` = **0.0508-0.3048 mm/tooth**. The chart's own APPLICATION guide names **52-200/57-200** as the `GOOD` choice for both *Single Pass* and *Roughing*, and its 1/8 in value is `.003"-.005"` = 0.0762-0.1270 mm. The chart's lowest 1/8 in entry of any series (52-700, 64-000/65-000) is `.002"-.004"` = 0.0508-0.1016 mm | **No** — even its *lowest entry at this diameter* has a floor 1.88x the cell's *ceiling* |
| `gwizard_hwood` | nothing at this diameter. Cited URL is a soft 404 (§3); the successor publishes one 1/2 in figure and no diameter table | **No** — and cannot, at any diameter below 1/2 in |
| `fpl_wood_handbook` | nothing. The Wood Handbook has no machining chapter in either the 2021 (GTR-282) or 2010 (GTR-190) edition — the same finding that removed it from every drill cell (W6 audit R-14, 2026-08-04) | **No** — same mis-citation class, left in place on this row |

Verbatim from the retrieved Onsrud PDF (p. 115, PCT-19):

```text
APPLICATION              GOOD             BETTER          BEST
  Single Pass       52-200/57-200     60-300/60-350     60-100C
    Roughing        52-200/57-200     60-800/60-900     60-000

Recommended Chip Load per Tooth by Cutting Diameter (in)
 Series      Cut    1/16      3/32      1/8        5/32      3/16
 52-200/     1xD                     .003-.005 .003-.005 .004-.006
 52-700      1xD                     .002-.004
 64-000/     1xD  .001-.003         .002-.004
FORMULAS: Chip Load = Feed Rate / (RPM x # of cutting edges)
```

### 5.2 Where 0.013-0.027 actually came from

The Shapeoko-lineage community chart — the publication `shapeoko_wiki`
points at (after §2's relocation) — publishes, verbatim:

| Diameter | Soft Plastics | Soft Wood & Hard Plastics | **Hard Wood & Metals** |
|---|---|---|---|
| 1/16" | 0.05mm-0.075mm | 0.025mm-0.04mm | **0.013mm-0.025mm** |
| 1/8" | 0.05mm-0.13mm | 0.025mm-0.063mm | **0.013mm-0.025mm** |
| 1/4" | 0.1mm-0.254mm | 0.025mm-0.127mm | 0.025mm-0.05mm |

**0.013-0.025 is the cell's band, to the millimetre.** The cell's own
`rpm`, `axial_doc`, `radial_woc` and `plunge_feed` rows already cite
`shapeoko_wiki`; the `feed_per_tooth` row is the one that does not —
so the band's provenance is a source the cell holds and does not credit,
attributed instead to two vendor charts that contradict it and one that
has no machining content at all.

Note what that column is: it does **not** change between 1/16 in and
1/8 in, and it lumps hardwood with metals. It resolves neither diameter
nor wood density, so it cannot speak to Ipe (Janka 3510) specifically.

### 5.3 The engine's number is Freud's own band, Janka-derated

Retrieved 2026-08-13, *Freud — Router Bit Feed Rates and Speeds for CNC*
(2017-08-22), "CHIP LOADS FOR FREUD SOLID CARBIDE ROUTER BITS ONLY":

```text
  Tool                                         Hardwood   Softwood
Diameter
   1/8"                                      .002"-.005" .004"-.006"
```

`.002"-.005"` = **0.0508-0.1270 mm/tooth** — which is, to the digit,
the shipped LUT row the engine matched
(`data/vendor_lut/observations/freud_solid_carbide.json`,
`freud-solid-carbide-eighth-hardwood`: `chipload_min_mm_tooth 0.0508`,
`chipload_max_mm_tooth 0.127`).

Applying the two scaling factors A-6 measured on this cell
(`chipload_hardness_scale` 0.6062, diameter scale 0.9660):

```text
0.0508 x 0.6062 x 0.9660 = 0.029748   (A-6 measured band min 0.0297)
0.1270 x 0.6062 x 0.9660 = 0.074370   (A-6 measured band max 0.0744)
```

The matched vendor band reproduces exactly. **The band the engine judges
this cell against is Freud's own published Ø1/8 in hardwood band, scaled
down 39 % for Ipe's density** — and the cell's commanded 0.0360 mm/tooth
sits inside it, at 1.21x the derated minimum. There is no engine defect
on this row to find.

(Two fpt figures circulate for this cell and they are not the same
measurement: **0.0360** is what the *matrix cell* commands, with its
`woc_mm`/`doc_mm` resolved `free`; **0.0437** is A-6 §4.1's separate
`feeds::calculate` probe at Ø3 2F pocket *roughing*. Both sit inside the
same derated band. This document uses 0.0360 throughout, because that is
the number the cell's verdict is computed from.)

Two further independent charts cap the same cell at the same place:
Cutter Shop 1/8 in hardwood `.003"-.005"` (0.0762-0.1270 mm) and Onsrud
as above. **Three independent vendor/retailer charts agree that Ø1/8 in
hardwood chipload tops out at `.005"` = 0.1270 mm/tooth.**

### 5.4 The re-baseline (sanctioned under K-(e1))

```text
old:  min = 0.013   max = 0.027
      sources = ["onsrud_hwood", "gwizard_hwood", "fpl_wood_handbook"]

new:  min = 0.013   max = 0.127
      sources = ["shapeoko_wiki", "onsrud_hwood", "freud_router_cnc"]
```

Only the **ceiling** moves: **0.027 -> 0.127, a factor of 4.70**. The
floor is unchanged and is now, for the first time, correctly attributed.
Two sources are dropped for cause (`gwizard_hwood`: no diameter table;
`fpl_wood_handbook`: no machining content) and two added for cause.

Scope, verified rather than asserted — the *entire* value-level diff of
`cells.toml` for this wave is two lines:

```text
-max = 0.027
+max = 0.127
-sources = ["onsrud_hwood", "gwizard_hwood", "fpl_wood_handbook"]
+sources = ["shapeoko_wiki", "onsrud_hwood", "freud_router_cnc"]
```

(`git diff cells.toml | grep -E '^[-+](min|max|sources|mode|unit|hobby)'`
returns exactly those four lines and nothing else. Everything else added
to that file is comment.) **No other cell, and no other band on this
cell, was touched.**

The new band is a **union with a hole in it**, and the cell's TOML comment
says so: the community band tops out at 0.025 and the industrial band
starts at 0.0508 — a **1.88x gap with no overlap** — and no retrievable
chart resolves wood density, so none of them adjudicate an Ipe
recommendation. A narrower band on this cell would be false precision,
and the honest instrument is a wide band that says why it is wide.

**Consequence**: the commanded 0.0360 moves `Outside (+33.4 %)` ->
`Within`. The cell keeps reporting its two remaining honest failures,
`axial_doc +20.0 %` and `plunge/feed -61.1 %`, at moderate.

### 5.5 What this does to A-6's finding

A-6 diagnosed *"a literature-vs-shipped-LUT disagreement about Ø3
hardwood chipload, roughly 2x wide, that no side of the engine can
resolve"*. That stands as a real disagreement, but it is **re-attributed**:
it is a **hobby-community-vs-industrial-vendor** disagreement (1.88x,
community low), not literature-vs-engine. The shipped LUT agrees with
the industrial vendor charts exactly; the cell was carrying the hobby
number under vendor citations. The engine is exonerated on this row.

Not re-opened: whether a hobby-machine cell *should* be judged against a
hobby chart or a vendor chart. That is a matrix-policy question across
many cells, not a source question, and S-2 does not rule it.

---

## 6. Incidental finding — `onsrud_doc_rule` asserts a rule its document does not contain

The row was named `"Onsrud DOC Rule (1.5-4.5x tool diameter
pre-derate)"`. A full-text read of the cited PDF returns **zero**
occurrences of `1.5`, `4.5`, or any depth-of-cut band. What the document
does print, verbatim, is a chip-load reduction *keyed to* depth of cut:

```text
DEPTH OF CUT: 1 x D Use recommended chip load
              2 x D Reduce chip load by 25%
              3 x D Reduce chip load by 50%
```

That is a rule about **chip load**, not about how deep to cut. Freud's
CNC chart prints the identical 1x/2x/3x reduction, so the reduction
itself is doubly attested — the `1.5-4.5x` DOC band is what has no
source. **54 citations**, most on `axial_doc` bands. The
`name` and `notes` are corrected here; **no `axial_doc` band is
re-derived** (out of S-2 scope). Ledgered **DR-DOCRULE**.

---

## 7. `dapra_rctf` — numerically self-verifying after the URL repair

The stored closed form and DAPRA's published form are algebraically
identical (max deviation 4.4e-16 over `ae/D` in
{0.05, 0.10, 0.20, 0.35, 0.49}):

```text
stored:     RCTF = D / (2 * sqrt(ae * (D - ae)))
published:  RCTF = 1 / sqrt(1 - (1 - 2*ae/D)^2)
```

and it reproduces DAPRA's own printed multiplier chart:

| WOC | computed | DAPRA chart | ratio |
|---:|---:|---:|---:|
| 35 % | 1.0483 | 1.05 | 0.9984 |
| 10 % | 1.6667 | 1.70 | 0.9804 |
| 5 % | 2.2942 | 2.30 | 0.9975 |

Stated condition, verbatim: *"If you're milling at less than 50% of your
cutting tool diameter without leveraging"* chip thinning, productivity is
being left on the table. `gold` is now earned on this row rather than
asserted; it carries 38 citations.

---

## 8. New ledger rows

| id | item | owner | re-open condition |
|---|---|---|---|
| **DR-SOFT404** | A `citation_url` can return HTTP 200, no redirect, and serve a home-page shell with the cited article gone (`cnccookbook.com/feeds-speeds-wood/`, found 2026-08-13). Neither the offline shape check nor a status probe can detect this — only reading the served content can. This makes DR-P3's "opt-in URL liveness check" insufficient as specified | whoever takes DR-P3 | immediately, if DR-P3 is scoped as a status check |
| **DR-GWIZ** | The three `gwizard_*` rows are cited on 12 `feed_per_tooth` and 21 `rpm` bands. The live successor publishes four 1/2 in figures and **no diameter table**, so none of those bands below 1/2 in are supported by this row. URLs repaired, bands untouched | a matrix-content wave | none — the rows now say so in their notes |
| **DR-DOCRULE** | `onsrud_doc_rule`'s `1.5-4.5x D` DOC band is absent from the cited document (§6). 54 citations, most on `axial_doc`. Name and notes corrected; no band re-derived | a matrix-content wave | any `axial_doc` recalibration |
| **DR-WSCITE** | 20 `feed_per_tooth` bands cite `whiteside_chipload`, a chart that does not exist. Zero of them cite it as sole source, so the key can be dropped from all 20 without leaving a band uncited | next intake | none — it is a citation-hygiene cleanup, not a number |
| **DR-403** | Four rows (`amana_spektra`, `amana_vbit`, `plastic_machining_guide`, `woodweb`) 403 to both curl and a browser-shaped fetcher, and two (`shaw_*`) return 202 behind an OUP bot challenge. Their `url_verified` is deliberately **not** advanced. `amana_spektra` alone is cited 142 times (21 of them `feed_per_tooth`) | the next refresh | a retrieval path that clears the bot-blocks |

**DR-URL and DR-WS are discharged.** DR-P3 is **not** taken (no network
I/O was added to any test binary) and is now amended by DR-SOFT404.

---

## 8b. Pre-registered prediction, written before the after-capture ran

Recorded here so the capture can falsify it rather than confirm a story
written afterwards:

1. `run_literature_matrix` **passes** (`moderate` does not block CI —
   `verdict::Severity::blocks_ci` gates on `Major | Critical`).
2. The Ipe cell's `feed_per_tooth` row moves
   `Outside 0.0360 > max 0.0270 (+33.4 %)` -> **`Within`**.
3. The cell's overall verdict **stays `moderate`**, and its summary line
   changes owner — from `feed_per_tooth` to **`axial_doc`
   (0.6000 > max 0.5000, +20.0 %)**, because `rollup` keeps the *first*
   row that reaches the maximum severity and `axial_doc` precedes
   `plunge_feed` in `ExpectedBands`.
4. The freshness report shows **no `warn` and no `stale` rows**, and the
   *never-confirmed links* list shrinks from **9 keys to 3** — losing
   `cutter_shop`, `toolgrit`, `shapeoko_wiki`, `dapra_rctf`,
   `kennametal_metals`, `kennametal_chipload`, and keeping only the
   403-blocked `amana_spektra`, `amana_vbit`, `woodweb`.
5. The citation audit reports **zero missing citations** (no source key
   was removed — the only key delta is `+freud_router_cnc`).
6. All 7 `_litmatrix_*` sentries stay green; none of them reads
   `cells.toml`, so the band move cannot reach them.

**Outcome: 6 of 6 confirmed, none falsified.** Captures in
`artifacts/s2/{litmatrix_before,litmatrix_after,litmatrix_sentries_after}.txt`.

The Ipe cell, before and after, from the same instrument:

```text
before:  fpt        Outside   0.0360 > max 0.0270 (+33.4%)
         verdict: moderate (fpt: 0.0360 > max 0.0270 (+33.4%))

after:   fpt        Within    0.0360 ∈ [0.0130, 0.1270]
         verdict: moderate (axial_doc: 0.6000 > max 0.5000 (+20.0%))
```

Every other row on the cell is unchanged, including the two that still
report honestly (`axial_doc` +20.0 %, `plunge/feed` −61.1 %) and the
K-(e2) relative anti-test, which stays `Within`.

Freshness report, before and after:

```text
before:  today=2026-08-13  total=32  fresh=32  warn=0  stale=0
         citation_url clock: 23 of 32 rows have a confirmed link; 9 never confirmed
         never-confirmed: amana_spektra, amana_vbit, cutter_shop, dapra_rctf,
                          kennametal_chipload, kennametal_metals, shapeoko_wiki,
                          toolgrit, woodweb

after:   today=2026-08-13  total=33  fresh=33  warn=0  stale=0
         citation_url clock: 30 of 33 rows have a confirmed link; 3 never confirmed
         never-confirmed: amana_spektra, amana_vbit, woodweb
```

Citation audit, both runs: `all citations resolve to known sources`.
Sentries: 7 binaries, **26 tests, all green**.

**Whole-matrix scope proof.** Diffing the two full reports over all 56
cells, the *only* differences outside the freshness block are four lines,
all on the Ipe cell — its `fpt` row (text, verdict and JSON `reason`),
and the cell summary. **No other cell moved at all.**

Method note, stated because it matters: both captures were produced by
running the already-built test binary
(`target/debug/deps/literature_matrix-69d8fb92d462ce2c`) directly rather
than through `cargo test`, because a parallel wave held the single Cargo
slot. That is sound *here specifically* — `cells.toml` and `sources.toml`
are read at **runtime** via `CARGO_MANIFEST_DIR`, not compiled in, and
this wave's `git diff` touches those two TOML files and no Rust source,
so the binary cannot be stale with respect to the change under test. The
"before" arm was produced by materialising the two files from `HEAD`
(`git show HEAD:…`) and restoring afterwards — never `git stash`, which
would have mutated a tree two other agents were working in.

---

## 9. NOT EXERCISED

- **DR-P3** — opt-in URL liveness checking in a test binary. Not taken;
  the decision it waits on ("is network I/O in a test binary acceptable
  at all") is not S-2's to make, and DR-SOFT404 changes its
  specification anyway.
- **`hobby_derate`** — the field is deserialized (`cell.rs:187`) and read
  by nothing. The Ipe cell carries `hobby_derate = 0.7` on the band this
  wave re-baselined; it is documentary, so the TOML `min`/`max` are the
  band as evaluated and no derate arithmetic was applied. Noted, not
  changed.
- **No `axial_doc`, `radial_woc`, `rpm` or `plunge_feed` band was
  re-derived** on any cell, including the Ipe cell's own three remaining
  Outside/Edge rows. S-2 refreshed sources and executed one ruled
  re-baseline; it did not re-derive the matrix.
- **`G-IPE-PLUNGE`** (the Ipe cell's `plunge/feed -61.1 %`) is untouched
  and still open. It is not a chipload question and this wave gives it no
  new evidence — except one negative datum worth having: its two cited
  sources are `shapeoko_wiki` and `vectric_default`, and the relocated
  Shapeoko publication does publish the fraction it is cited for,
  verbatim `"30% to 40% of the feedrate for woods"`. So the cell's
  `0.30-0.50` band has a live source for its floor and the -61.1 %
  finding is **not** explained by a dead citation.
