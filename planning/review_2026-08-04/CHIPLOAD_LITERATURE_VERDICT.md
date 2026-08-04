# Chipload literature verdict — T4.1 (column convention) and T3.4/T4.3 (one law each)

Date: 2026-08-04
Wave: **B-lit** (second tech-debt programme, branch `experiment/adaptive-spiral`)
Basis: Checkpoint B ruling 2026-08-04 — Q1 *"Verify in literature, then convert"*,
Q4 *"one literature task"* (`planning/review_2026-08-04/ORCHESTRATION_LOG.md:23`).
Questions: `planning/review_2026-08-04/FEEDS_CENSUS.md` §10 item 1 (T4.1), §8 T3.4 / T4.3 / T4.6.
Policy binding this document: `.claude/skills/refresh-lit-matrix/SKILL.md` step 2 —
a source is either **re-verified against the artefact the claim rests on**, or
**replaced**, or recorded as **NOT VERIFIED**. Nothing is inferred silently.

Status: **research only. This document changes no code, no data file, and no
`CREDITS.md` entry.** §5 is a proposed `CREDITS.md` block for the implementation
wave to apply *with* the code, per the Checkpoint B ruling.

Cargo commands run this wave: **none.** No claim below requires a build; every
numeric result is either arithmetic over file-resident constants (shown inline
with its inputs) or a regression over the shipped LUT data files (method in §4.0).

---

## 0. Executive summary

**Q1 — the column convention. Verdict: ADVANCE PER TOOTH, for every source
family in the shipped LUT. Confidence: HIGH for six of seven families, MEDIUM for
one.** Four vendors (Onsrud, Freud, Amana, Garr) print the defining identity
`chip load = feed rate ÷ (RPM × cutting edges)` on the same page as the column.
On the two Amana charts that publish both an IPM column and a chip-load column,
the identity is **numerically self-verifying** — including on the exact chart
behind the live B3 row. No wood source in the LUT publishes a radial-engagement
condition for the chipload column at all; all three that state a condition state
an **axial** one (`ap = 1 × D`), with an identical published derate table
(100 % / 75 % / 50 % at 1 / 2 / 3 × D) that the crate already implements exactly.

**Consequence for F-1: the gate's comparison is invalid as printed, and the fix
is a deletion, not an inversion.** The census proved `cl_norm = fz · f_lut`
(FEEDS_CENSUS §4.3) — the sample arc cancels. Dividing the observation by
`f_lut` therefore recovers `fz` at the effective feed *exactly*. Under the
advance-per-tooth verdict the correct observed quantity is
`fz_eff = effective_feed ÷ (rpm · flutes)`, so the D9 arc normalisation and the
chip-geometry step should be **removed** from the chipload gate rather than
inverted. That also removes the gate's dependence on the LUT `ae` bands — which
matters, because §2.3 shows those bands are **repo-authored application windows,
not vendor-published measurement conditions**. The 2.4×–40.4× correction the
census sized is real, but it is a correction *out of* a quantity that has no
primary-source standing on either end.

**Q2 — one diameter law: `chipload ∝ D^0.61`.** Adopt the formula path's existing
exponent (`machine.rs:40`) in the LUT path, retiring `vendor_lookup.rs:265-268`'s
`^1.0`. Row-count-weighted central estimate regressed from the shipped vendor
charts themselves: **0.52**; per-family range **0.23–1.25**; Onsrud's own two wood
charts give **0.37** across 50 series-fits at r² > 0.95. 0.61 sits inside that
spread and is what the single widest-span series in the LUT measures (Amana
Spektra, Ø0.79–12.7 mm, 16× span: 0.586 / 0.619, r² 0.89–0.97). **Piecewise was
tested and is NOT supported** — see §4.2.

**Q2 — one hardness law: `chipload ∝ Janka^-0.5`.** Same shape: adopt the formula
path's *effective* exponent in the LUT path, retiring `vendor_lookup.rs:306-337`'s
`^1.0`. **The census's C-7 is wrong on the formula side** and this is the single
most consequential correction in this document: `feeds/mod.rs:835` applies
`(1/feed_scale)^1.26`, but `feed_scale = (janka/600)^0.4` (`material.rs:688-692`),
so the formula path's exponent **on the Janka axis is 0.4 × 1.26 = 0.504**, not
1.26. The two laws are `^1.0` and `^0.504`, not `^1.0` and `^1.26`. Independent
support: USDA FPL Wood Handbook Table 5–11a gives side hardness ∝ G^1.49 (SW) /
G^2.09 (HW) while compression-parallel and shear-parallel — the strength axes a
cutting force actually loads — go as G^0.85–1.13; constant force per tooth
therefore implies `fz ∝ Janka^-0.48 … ^-0.67`, bracketing 0.5.

**Both recommendations are one-sided.** Each says "the LUT path adopts the
formula path's exponent." Neither moves the formula path: the diameter exponent
is already 0.61, and a unified `Janka^-0.5` differs from the shipped composite
`^0.504` by **≤ 0.71 % across the entire calibrated Janka band (300–3510 lbf)**.
The blast radius is `vendor_lookup::{diameter_scale_factor, hardness_scale_factor}`
and nothing else.

**Direction, stated plainly.** Both changes **raise** the band when a row is
transferred down in diameter or up in hardness, and **lower** it when transferred
the other way. On the live B3 row the derated band goes 0.00458–0.00916 →
0.00732–0.01464 mm/tooth. That is the *opposite* of a safety tightening on small
tools, and it is what the vendors' own charts say: `^1.0` under-predicts
permissible chipload on small tools relative to every vendor family in the LUT
except Freud, and under-predicting chipload commands too-slow feed, which in wood
is the rubbing-and-burning failure mode, not a conservative margin.

---

## 1. Method and evidence standard

Per `/refresh-lit-matrix` step 2, each source family below is resolved to one of:

- **VERIFIED** — the artefact the claim rests on was retrieved and the load-bearing
  wording quoted verbatim from it.
- **VERIFIED (publisher-level)** — the wording was retrieved from a different
  document by the same publisher carrying the identical boilerplate block; the
  specific chart behind a given LUT row was not individually retrieved.
- **NOT VERIFIED** — recorded in §7 with exactly what was attempted.

Forum posts, calculator sites and AI summaries are **not** evidence here. Where a
search-engine summary was the only thing available (Helical, Sandvik hm), the row
says so and the claim is downgraded accordingly.

PDFs that `WebFetch` could not text-extract were downloaded by the same tool and
extracted locally with `pdftotext -layout`; every quotation marked VERIFIED below
comes from that extracted text, not from a model summary of the PDF.

---

## 2. Q1 — the column convention, per source family

### 2.1 Verdict table

Families are the seven `source_vendor` values in
`crates/rs_cam_core/data/vendor_lut/observations/*.json` (252 rows, counted
2026-08-04). "Rows" = observations carrying that vendor.

| # | Source family | Rows | Verdict | Confidence | Evidence class |
|---|---|---:|---|---|---|
| V-1 | **LMT Onsrud** | 67 | `advance_per_tooth` | **HIGH** | defining identity printed on the chart, VERIFIED verbatim |
| V-2 | **Freud** | 14 | `advance_per_tooth` | **HIGH** | identity + worked example printed on the chart, VERIFIED verbatim; carries an informal "thickness" gloss (§2.2) |
| V-3 | **Amana Tool** | 122 | `advance_per_tooth` | **HIGH** | identity printed on the chart AND numerically self-verifying against the chart's own IPM column, VERIFIED verbatim on the chart behind the live B3 row |
| V-4 | **Garr Tool** | 16 | `advance_per_tooth` | **HIGH** | publisher's own chip-thinning page distinguishes programmed feed from resulting chip, VERIFIED verbatim; per-row `ae` conditions are ≥ 0.5 D where `fz ≡ h_ex` |
| V-5 | **Helical Solutions** | 2 | `advance_per_tooth` (chip-thinning **compensated**) | **MEDIUM** | guidebook PDF not retrieved (§7 N-4); direction corroborated by two independent publishers |
| V-6 | **IDC Woodcraft / Millmage** | 10 | `advance_per_tooth` | **HIGH (by construction)** | repo's own rows declare `row_kind: "derived"`, "chipload back-calc from feedrate/rpm/flutes" |
| V-7 | **Whiteside** | 21 | `advance_per_tooth` | **MEDIUM** | no vendor chart exists; 13 of 21 rows are a Fusion 360 `.tools` preset whose schema field is `feedPerTooth`. Source file not retrieved (§7 N-3) |

**No family reads `mean_chip_thickness`. No family is `ambiguous` on its
operational definition.** The one genuine ambiguity found is prose-level and
resolves against the mean reading anyway (§2.2).

### 2.2 The wording, verbatim

**V-1 Onsrud** — `Hard Wood Cutting Data Recommendations`,
<https://www.onsrud.com/images/Hard%20Wood.pdf> (p. 115), and
`Soft Wood Cutting Data Recommendations`,
<https://www.onsrud.com/images/Soft%20Wood.pdf> (p. 114). Retrieved and
text-extracted 2026-08-04.

> Recommended Chip Load per Tooth by Cutting Diameter (in)

> FORMULAS: Chip Load = Feed Rate / (RPM x # of cutting edges)
> Feed Rate (IPM) = RPM x # of cutting edges x chip load
> Speed (RPM) = Feed Rate / (# of cutting edges x chip load)

> DEPTH OF CUT: 1 x D Use recommended chip load
> 2 x D Reduce chip load by 25%
> 3 x D Reduce chip load by 50%

The column is *defined* by the kinematic identity. The only stated qualifier is
axial. The word "thickness" does not appear on either chart.

**V-2 Freud** — `Router Bit Feed Rates and Speeds for CNC` (solid-carbide chart,
2017-08-22),
<https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf>.
Retrieved and text-extracted 2026-08-04.

> Chip Load = Feed Rate ÷ (RPM x number of flutes)
> Feed Rate = RPM x number of flutes x chip load
> RPM = Feed Rate ÷ (number of flutes x chip load)
> Note: Feed Rate will be expressed in Inches per Minute

> Recommended* Chip Loads, based on cut depth equal to bit diameter

> The Chip Load is the size (thickness) of the chip produced as your bit cuts.

Freud is the one family carrying a **thickness gloss**. It is settled against the
mean reading on three grounds, all internal to the document: (a) the operational
definition immediately below it is the kinematic identity; (b) a worked example
follows — *"Feed Rate = 18,000 x 2 x .019". Therefore, your Feed Rate should be
684 inches per minute"* — which only closes if the column is a commanded advance;
(c) if it were any chip thickness it would be the **maximum**, since the stated
reference is a full-diameter-depth cut. A mean-chip reading is unsupported by any
sentence in the document. Verdict `advance_per_tooth`, confidence HIGH, gloss
recorded.

**V-3 Amana Tool** — `ZrN-Coated and Uncoated 2D/3D Carving CNC Solid Carbide
Router Bits` (the chart behind `source_id = amana_zrn_3d_profiling`, i.e. the
matched row on the live B3 op), retrieved via the ToolsToday mirror
<https://toolstoday.com/content/ProductFile/Attachments/ZrN-3D-Profiling-Feed-Chip-Load-Chart.pdf>
and text-extracted 2026-08-04 (amanatool.com returns HTTP 403 to automated
fetch — §7 N-1):

> Operating RPM: 18,000

> IPM*  |  Chip Load Per Tooth (Based on 18,000 RPM)
> * IPM Inches per minute

> To find Feed Rate = RPM x # of flutes x chip load
> To find Chip Load = IPM / (RPM x # of Flutes)

> Depth of Cut: 1 x D Use recommended chip load
> 2 x D Reduce chip load by 25%
> 3 x D Reduce chip load by 50%

**This chart is numerically self-verifying**, which is the strongest evidence in
this document. It publishes IPM and chip load side by side at a fixed 18,000 RPM,
so the identity can be checked without trusting the footnote:

| chart row | printed IPM | flutes | IPM ÷ (18000 × flutes) | printed chip load |
|---|---|---|---|---|
| 2-flute ball nose, 6 mm–1/4" wood | 250–320 | 2 | 0.00694–0.00889 in | **0.007"–0.009"** |
| 3-flute ball nose, 6 mm wood | 215–320 | 3 | 0.00398–0.00593 in | **0.004"–0.006"** |
| 3-flute ball nose, 3/8" wood | 320–430 | 3 | 0.00593–0.00796 in | **0.006"–0.008"** |
| 2-flute flat bottom, 1/8" wood | 110–180 | 2 | 0.00306–0.00500 in | **0.003"–0.005"** |

The column *is* `feed ÷ (rpm · flutes)`, to the chart's own printed precision.
No arc, no engagement, no chip-geometry term can be present in a number that
reproduces the kinematic identity to three significant figures.

The identical footer block was independently confirmed on two further
Amana charts retrieved from separate mirrors — the Aluminum O-Flute chart
(<https://toolstoday.com/content/ProductFile/Attachments/Aluminum-O-Flute-Speed-Chart.pdf>):
*"To find Feed Rate: RPM x # of flutes x chip load / To find Chip Load: IPM /
(RPM x # of Flutes)"*, and the Spektra Plastic O-Flute v11 chart
(<https://www.woodworkerexpress.com/skin/common_files/files/specs/amanatool/Spektra-Plastic-O-Flute-Speed-Chart-v11.pdf>):
*"Operating RPM: 18,000 / Depth of Cut: 1 x Tool Diameter †"*, same four
calculations, same three-line depth-of-cut derate. The convention is therefore
established at the **publisher** level for all 122 Amana rows; the nine
individually-unretrieved Amana charts are listed in §7 N-1.

**V-4 Garr Tool** — `Chip Thinning`, <https://www.garrtool.com/knowledge-base/chip-thinning/>,
retrieved 2026-08-04:

> Chip Load (or Maximum Chip Thickness) is one of the most important parameters
> for achieving a stable and productive milling process.

> Radial chip thinning occurs when the width of cut is less than 50% of the
> cutter diameter.

> when feeding an endmill at 50% radial engagement … the chip thickness holds
> true to the feed rate programmed

> the feed rate is increased to maintain a co[n]stant chip thickness relative to
> the radial depth of cut

Garr equates chip load with **maximum** chip thickness — and only at
`ae ≥ 0.5 D`, where `h_ex = f_z` identically. Below that, chip load remains the
*programmed* quantity and the chip thins away from it. This is an
advance-per-tooth reading with a stated reference engagement, and it is
categorically **not** a mean.

The Garr rows in the LUT carry that reference explicitly:
`ae_rule = "Radial (ae) = 1xD for slotting"` and
`"Profiling Radial = .5xD (up to 50% of Dia.)"` — both at or above the threshold.

**V-5 Helical Solutions** — Machining Guidebook 2016. The guidebook PDF itself
was not retrieved (§7 N-4). The two LUT rows encode
`ae_rule = "HEM RDOC = 0.200 in = 40% x D"` at `chipload = 0.235 mm` and
`"Traditional RDOC = 0.250 in = 50% x D"` at `0.247 mm`. A number published at
40 % radial that is *within 5 % of* the 50 % number is a chip-thinning-compensated
programmed feed per tooth; a mean chip thickness at 40 % radial would be roughly
half the 50 % value. The direction is corroborated by two independent publishers
(Garr above; Harvey Performance, *"Combat Chip Thinning"*,
<https://www.harveyperformance.com/in-the-loupe/combat-chip-thinning/>).
Confidence MEDIUM on wording, HIGH on direction.

**Note for the implementation wave:** V-5 is the one family where applying
`f_lut` would **double-thin**. Helical's number has already been raised for
chip thinning; normalising it down again by a mean-chip factor computed from the
same 40 % engagement compounds the correction twice in the same direction.

**V-6 IDC Woodcraft / Millmage** — no external verification needed and none
claimed. The repo's own rows state the provenance:
`"row_kind": "derived"`, `"material_label": "… chipload back-calc from
feedrate/rpm/flutes"`, `"evidence_grade": "c"`
(`observations/idcwoodcraft_millmage.json`). The quantity is
`feed ÷ (rpm · flutes)` because the repo computed it that way. This is
**repo-internal provenance, not a primary source** — it establishes the unit with
certainty and establishes the *value's* authority not at all.

**V-7 Whiteside** — 13 of 21 rows come from a Fusion 360 `.tools` library
(`whiteside_fusion360_tools_2019-10-23`), all `evidence_grade: "c"`,
`row_kind: "fallback"`, all `chipload_max_mm_tooth = 0.1016` (= 0.004") at every
diameter from 3.175 to 6.35 mm. The Fusion tool-library field is `feedPerTooth`,
which Autodesk documents as the per-tooth feed the CAM system multiplies by
spindle speed and tooth count to produce the F-word
(<https://www.autodesk.com/products/fusion-360/blog/machining-fundamentals-introduction-to-speeds-and-feeds/>).
The `.tools` file itself was not retrieved (§7 N-3). Confidence MEDIUM —
sufficient to rule out a mean-chip reading, insufficient to call the rows
authoritative for anything else.

### 2.3 The finding the brief did not ask for, and the one that matters most

**No wood source in the shipped LUT publishes a radial-engagement condition for
its chipload column.** Verified by exhaustive reading of every wood chart
retrieved in full this wave — Onsrud Hard Wood, Onsrud Soft Wood, Freud solid
carbide, Amana ZrN 2D/3D Carving (both pages), Amana Aluminum O-Flute, Amana
Spektra Plastic O-Flute. All six state an **axial** condition (`1 × D`) and an
axial derate table. None mentions stepover, width of cut, radial engagement, or
`ae`. Only the two **metal** families (Garr, Helical) state a radial condition,
and both state one at or above `0.5 D`.

So where do the LUT's `ae_min_mm` / `ae_max_mm` values come from? From the repo.
Every `ae`-bearing row carries an `ae_rule` string, and on the wood rows those
strings are **application windows the ingest authored**, not transcriptions:

| `ae_rule` (verbatim from the data files) | rows | what it actually describes |
|---|---:|---|
| `"scallop driven"` | 19 | the stepover a scallop-height target implies — a *finishing pass parameter* |
| `"10% to 30%D"`, `"13% to 28%D"`, `"2% to 8%D"`, … (10 distinct strings) | 23 | a curated stepover window per operation family |
| `"width-at-depth"` | 8 | V-bit geometry, not an engagement recommendation |
| `"35% to 80%D"`, `"36% to 72%D"`, … | 6 | spoilboard surfacing overlap |
| `"Radial (ae) = 1xD for slotting"`, `"HEM RDOC = 0.200 in = 40% x D"`, … | 4 | **the only vendor-published ones** — both metal families |

`lut_nominal_arc_rad` (`chipload.rs:127`) computes the D9 normalisation arc from
`(ae_min + ae_max)/2`. On the live B3 row that midpoint is 0.265 mm — 8.3 % of the
3.175 mm tool — because the rule is `"scallop driven"`, i.e. it is the stepover a
*finish pass* runs at. It is not, and was never claimed to be, the engagement at
which Amana measured the chipload column. Amana's chart states the reference
condition explicitly and it is axial.

**Therefore `f_lut` is not a unit conversion between two published quantities.**
It is a factor computed from a repo-authored stepover window and applied to a
vendor number published under a different, axial condition. Its 2.4×–40.4× spread
across the 65 `ae`-bearing chipload rows (FEEDS_CENSUS §9.2) is the spread of the
*ingest's* stepover conventions, not of any vendor disagreement. This is the
strongest argument for deletion over inversion, and it is new relative to the
census.

### 2.4 A bonus confirmation the wave was not looking for

Three independent publishers print the **same** axial derate table:

> 1 x D Use recommended chip load / 2 x D Reduce chip load by 25% / 3 x D Reduce chip load by 50%

— Onsrud (both wood charts), Freud (*"If Cut Depth is 2X the bit diameter, reduce
the Chip Load by at least 25%"*), and Amana (every chart retrieved). The crate's
`feeds::geometry::doc_derating_scale` (`geometry.rs:143-153`) implements
`1.000 / 0.750 / 0.500` at `ratio = 1 / 2 / 3` with linear interpolation between
and a clamp at 0.5 — an exact match to all three, with the vendors' silence
between breakpoints filled by interpolation. **C-5's shared helper is correct and
now has triple primary-source backing.** The open C-5 defect is which *diameter*
goes in the denominator (FEEDS_CENSUS §2.4), not the scale itself; and the
vendors settle a second question while they are here — the ratio's numerator is
unambiguously the **axial** depth of cut.

---

## 3. Q1 consequence — what the verdict does to F-1

### 3.1 The conversion is a deletion

FEEDS_CENSUS §4.3 established `cl_norm = fz · f(arc_sample) · f_lut / f(arc_sample)
= fz · f_lut`, pinned executably by
`crates/rs_cam_core/tests/feed_explanation_snapshot_b3.rs` (commit `5f7bb25`).
Under the advance-per-tooth verdict the band's unit is `fz`, so:

```
correct observed quantity = cl_norm / f_lut
                          = fz  (at the effective/predicted feed)
                          = effective_feed_mm_min / (rpm · flute_count)
```

The chip-geometry call and the D9 renormalisation cancel to the identity. They
can be removed rather than inverted, which is strictly better because it drops
the gate's dependence on the un-sourced `ae` bands of §2.3. `effective_feed_for_sample`
(`tool_load/mod.rs:63`) already exists and is already honoured by all three gates
(FEEDS_CENSUS §0.3).

### 3.2 What the live B3 op actually was

All inputs file-resident; `f_lut = 0.0804824`, `feed_ratio = 0.12820`, band
`0.00458–0.00916` mm/tooth, commanded fpt `N1 = 0.0714`.

| quantity | value (mm/tooth) | vs band |
|---|---:|---|
| gate observed **today** (arc-mean chip) | 0.000737 | **16 % of band *minimum*** → `Within` + burn advisory |
| **corrected** observed (`fz` at effective feed) | **0.009153** | **99.9 % of band *maximum*** |
| commanded fpt (`N1`) | 0.0714 | **780 % of band maximum** |

The gate reports an operation sitting **at its ceiling** as sitting **six times
below its floor**, and issues a burn advisory for it. One percent more effective
feed and the same op is `Exceeds(High)`. This is the single most useful sentence
in this document for the operator: the correction does not merely rescale a
number, it **inverts which side of the band the op is on**.

Note also that the census's valid-but-discarded comparison (N1 vs N4, "7.8× the
band max", FEEDS_CENSUS §0.2 finding 2) and the corrected gate reading are the
*same statement at two feeds* — 7.80× commanded, 1.00× achieved. Both should be
reported, labelled, and it is their **ratio** (the −87 % kinematic throttle) that
is the operator-actionable fact, not either alone.

### 3.3 What becomes valid and invalid

| comparison | today | under the verdict |
|---|---|---|
| gate observed vs LUT band | **INVALID** (chip vs advance) | **VALID** once the observation is `fz_eff` |
| narration nominal (N1) vs band | VALID, never surfaced (P-10) | VALID and now *comparable to the gate's own number*, differing only by the disclosed feed ratio |
| Suggest's `arc_fit_ratio_for_op` (C-13/F-5) vs gate | two different models, 14.5× apart | the gate's model becomes `fz_eff`, i.e. `nominal × feed_ratio` — **`arc_fit_ratio_for_op` should be retired, not re-keyed**, since the operation-type table predicts a quantity that no longer exists |
| `RUBBING_FLOOR_MM_TOOTH` (0.025) vs band | contradictory (2.73× band max) | **still contradictory** (1.71× band max after the Q2 laws land) — T3.3/T4.2 is **not** resolved by this verdict and remains a live Checkpoint item |
| `_litmatrix_rubbing_floor_clamp` pinning 0.025 against `shaw_chipload` | asserted on the advance convention | **now correct by evidence** rather than by assumption — the pin stands, gap 3 of FEEDS_CENSUS §7 closes |
| optimizer retarget multiplier (`retarget/chipload.rs:109`, `target / gate-observed`) | denominator is an arc-mean chip | denominator becomes `fz_eff`; **the multiplier moves by `1/f_lut` on every `ae`-bearing row** and the optimizer's targets must be re-derived, not carried over |

### 3.4 What the conversion approval will need

The Checkpoint B ruling requires a **measured verdict-flip table on committed
fixtures** before any conversion ships. Minimum contents, derived from the above:

1. **Per-fixture, per-toolpath before/after rows**: `ChipBoundsSource`, `row_id`,
   observed (old unit), observed (new unit), band min/max, verdict, `burn_advisory`
   present/absent, `Severity`. A flip is any change in verdict *or* in advisory
   presence — §3.2 shows the second can move without the first.
2. **Coverage of both `f_lut` extremes**, not just the median: at least one
   fixture on a row near `f_lut = 0.0247` (`garr-a3-alum-finish-6000-flat-3f`)
   and one near `0.4117` (`amana-facing-softwood-face-22000-2f`), because a
   16.6×-varying correction cannot be validated at its median.
3. **At least one `VendorLutMissingAe` fixture** — 176 of 252 rows carry no `ae`,
   D9 is already disabled for them (`chipload.rs:522-523`), and their verdicts
   must be shown **unchanged**. If they move, the change did something other than
   what this document authorises.
4. **The B3 fixture re-run** (`feed_explanation_snapshot_b3.rs`) with its residual
   assertion intact; the identity must still close at ≤ 1 %.
5. **The five live feeds sentries** (FEEDS_CENSUS §9 item 4 — `wanaka_e2e_chipload_gate`
   is `#[ignore]`d and must not be counted): `chipload_advisory_disclosure_h4`,
   `chipload_formula_calibration`, `predicted_feed_gates_f035`,
   `sim_chipload_invariant`, `tapered_width_model_parity_c3`.
6. **The optimizer**, separately: §3.3's last row means retarget multipliers move
   even where no verdict flips. A verdict-flip table alone will not catch that.
7. **Ordering.** T3.1 before T3.3, T3.5 and every drill recalibration (FEEDS_CENSUS
   §8 ordering constraint) — unchanged by this document.

---

## 4. Q2 — one diameter law, one hardness law

### 4.0 Method

Both laws were regressed **from the shipped vendor data itself**, which is the
only primary-source quantification available: no vendor publishes an exponent.

- *Charts*: Onsrud Hard Wood and Soft Wood PDFs were text-extracted and parsed
  positionally into (series × diameter) cells — 26 and 24 series respectively,
  and 125 matched (series, diameter) hardwood/softwood pairs.
- *LUT*: all 252 observations in `data/vendor_lut/observations/*.json`, grouped by
  (`source_id`, `tool_subfamily`, `material_family`, `flute_count`, `pass_role`)
  so that series-to-series chipload differences cannot masquerade as a diameter
  effect. Band midpoints `(min+max)/2` were used; log–log OLS; r² reported.
- This grouping matters: an ungrouped Onsrud fit returns a *negative* exponent
  purely because the 60-000 and 60-200 series differ by 3× at the same diameter.
  Any future re-derivation that skips the grouping will reproduce that artefact.

### 4.1 Diameter law — recommendation

> **`chipload ∝ D^0.61`, applied on both paths. Regime of validity: diameter
> ratios within 0.5×–2× of the matched row. Beyond that the row must be reported
> as extrapolated *with direction*, not silently scaled.**

Measured per-family exponents (grouped as above; `n` = points, r² of the log–log fit):

| family | series fits | exponent range | median | typical r² | diameter span |
|---|---:|---|---:|---|---|
| **Onsrud** (own charts, both woods) | 50 | 0.20 – 0.66 | **0.37** | 0.95–0.999 | 1.6–50.8 mm |
| Onsrud (LUT subset) | 9 | 0.29 – 0.49 | 0.375 | 0.91–0.999 | 3.2–19.1 mm |
| **Amana** Spektra spiral plunge | 2 | 0.586 – 0.619 | 0.60 | 0.89–0.97 | **0.79–12.7 mm (16×)** |
| Amana ZrN 2D/3D carving | 2 | 0.488 – 0.924 | 0.71 | 0.85–0.94 | 3.2–6.4 mm |
| **Freud** | 3 | 1.05 – 1.25 | 1.09 | 0.97–0.998 | 3.2–12.7 mm |
| IDC Woodcraft (grade c, derived) | 1 | 0.230 | 0.23 | 0.79 | 3.2–12.7 mm |
| Whiteside Fusion360 (grade c, preset) | — | 0.0 (constant 0.1016 at all Ø) | 0.0 | — | 3.2–6.4 mm |

Row-count-weighted central estimate across the wood-relevant families: **0.52**.
Unweighted cross-family median: **0.40**.

**Why 0.61 and not 0.52 or 0.40.** Three reasons, in order:

1. **It changes one implementation, not two.** 0.61 is already
   `ChipLoadFormula::default().p` (`machine.rs:40`), so recommending it means the
   LUT path adopts the formula path and the formula path does not move. Choosing
   0.52 would move both, doubling the blast radius to buy a difference of
   `0.3005^(0.52−0.61) = 1.12×` at the most extreme scale in the live evidence —
   inside the between-series scatter of the measurement itself.
2. **It is what the widest-span series measures.** Amana Spektra spans 16× in
   diameter (Ø0.79 → 12.7 mm) with 10 points and r² 0.89–0.97, giving 0.586 (MDF)
   and 0.619 (softwood). Every other series spans ≤ 6× — over a short span the
   fitted exponent is dominated by rounding in the chart's own printed values.
3. **It sits inside the physical bracket, and the bracket is wide by nature.**
   Two limits bound the exponent from opposite ends. Deflection-limited: tool
   stiffness ∝ D⁴/L³ with L ∝ D gives stiffness ∝ D; force ∝ K_c·f_z·a_p ∝ f_z·D
   at the charts' own `a_p = 1 × D` reference; constant deflection ⇒ **p = 0**.
   Strength-limited: bending moment ∝ f_z·D², section modulus ∝ D³, constant
   stress ⇒ **p = 1**. Every measured family lands in [0, 1.25]; the vendors
   disagree because their charts encode different binding constraints at
   different tool series, not because one of them is wrong.

**Uncertainty, stated.** The exponent is a fitted summary of seven vendors who
span 0.23–1.25. It is not a physical constant and must never be cited as one. The
Freud family genuinely sits near 1.0 and is the only support for the LUT's current
law; it is also the smallest wood dataset (3 series, 3–4 diameters each) and its
1/8" hardwood cell is conspicuously conservative relative to its 1/4" cell (4× the
chipload for 2× the diameter), which alone drives its 1.25.

**Piecewise was tested and is NOT recommended.** The census's implicit worry — that
small tools obey a different law — has exactly one supporting series: Amana Spektra
softwood splits at Ø3.2 into p = 0.936 (below) and p = 0.244 (above). The *same
chart's MDF series at the same diameters* does not reproduce it (0.722 / 0.522),
and Onsrud's two sub-2 mm series (10-00 and 64-000/65-000, both down to 1.59 mm)
show no break (0.36, 0.63). One series in one material in one chart is a
coincidence, not a breakpoint. Declaring one would be false segmentation, which is
the same failure the brief forbids under the name "false unification".

**Effect on the shipped values** (`d_scale = query_Ø / row_Ø`, clamped 0.1–10):

| `d_scale` | today (`^1.0`) | proposed (`^0.61`) | band moves by | extrapolation flag today | …after |
|---:|---:|---:|---:|---|---|
| 0.30 (live B3: Ø1.0 tip vs Ø3.175 row) | 0.3005 | 0.4803 | **×1.60** | flagged | flagged |
| 0.50 | 0.500 | 0.655 | ×1.31 | flagged | flagged |
| **0.60** | 0.600 | 0.732 | ×1.22 | flagged | **NOT flagged** ⚠ |
| **0.71** | 0.710 | 0.812 | ×1.14 | flagged | **NOT flagged** ⚠ |
| 1.00 | 1.000 | 1.000 | ×1.00 | — | — |
| 1.40 | 1.400 | 1.228 | ×0.88 | — | — |
| 2.00 | 2.000 | 1.526 | ×0.76 | flagged | flagged |
| 3.00 | 3.000 | 1.955 | ×0.65 | flagged | flagged |

⚠ **A trap the implementation wave must handle.** `is_extrapolated` is computed on
the *applied* `total_scale` (`vendor_lookup.rs:349`, `|ln(d_scale · h_scale)| > ln 1.4`).
Softening the exponents shrinks that quantity, so rows in roughly
`d_scale ∈ [0.58, 0.71] ∪ [1.40, 1.72]` **silently lose their extrapolated flag** —
which downgrades `Confidence::Approximate` back to `Validated` and, via
`low_side_is_advisory` (`verdict.rs:632`), converts advisory low-side reports into
**hard `Exceeds(Low)` trips**. The flag measures *how far the row is from the
query* and must therefore be computed on the **raw ratios**, before the exponent
is applied. This is a behavioural change with no ruling behind it and must not be
allowed to ride along.

### 4.2 Hardness law — recommendation

> **`chipload ∝ Janka^-0.5`, applied on both paths, with the 600 lbf anchor the
> crate already uses. Regime of validity: the crate's calibrated Janka band
> (`JANKA_CALIBRATED_BAND_LOW/HIGH_LBF`, `material.rs`); outside it, refuse rather
> than extrapolate — which is what `feed_scale_factor` already does.**

**First: the census's C-7 is wrong on the formula side, and this changes the
question.** `feeds/mod.rs:835` reads `(1.0 / feed_scale).powf(cl.q)` with
`q = 1.26`, but `feed_scale` is not a hardness — it is
`(janka / 600).powf(0.4)` (`material.rs:689`, and identically for
`SolidWoodByJanka`, `Plywood`, `SheetGood`). Composing:

```
(1/feed_scale)^q = ((janka/600)^0.4)^-1.26 = (janka/600)^-0.504
```

**The formula path's exponent on the Janka axis is 0.504, not 1.26.** The two live
laws are therefore `^1.0` (LUT) and `^0.504` (formula) — a 2× disagreement on the
log axis, not the 1.26× the census reported. Any implementation that "unified on
1.26" would move the formula path by a factor of `janka^-0.756`, e.g. **2.8×** at
Ipe. This correction should be carried back into FEEDS_CENSUS §2.5 and §0.1 row C-7.

**Primary-source lineage for 0.5.** USDA Forest Service, *Wood Handbook — Wood as
an Engineering Material*, GTR-190 (2010), Chapter 5, **Table 5–11a** *"Functions
relating mechanical properties to specific gravity of clear, straight-grained
wood"* (retrieved and text-extracted 2026-08-04 from
<https://www.precisebits.com/PDF/USFS_mechanical_properties_of_wood.pdf>, the URL
already carried by the LUT manifest as `fpl_ch5_2010`):

| property (12 % MC) | softwoods | hardwoods |
|---|---|---|
| **Side hardness (N)** — the Janka axis | **8,590 G^1.49** | **15,300 G^2.09** |
| Compression parallel (kPa) | 93,700 G^0.97 | 76,000 G^0.89 |
| Shear parallel (kPa) | 16,600 G^0.85 | 21,900 G^1.13 |

Janka scales as G^1.49–2.09; the strength axes a cutting edge actually loads scale
as G^0.85–1.13, i.e. ≈ G^1.0. Holding force per tooth constant gives
`f_z ∝ 1/K_c ∝ G^-1.0`, hence:

```
f_z ∝ Janka^(-1/2.09) … Janka^(-1/1.49)  =  Janka^-0.48 … Janka^-0.67
```

**0.5 is at the conservative edge of a bracket derived from a gold-tier primary
source, using the crate's own Janka input variable.**

**Cross-check against what the vendors actually do** — and this is where the honest
uncertainty lives. Onsrud publishes the same 26 tool series in a Hard Wood chart
and a Soft Wood chart, giving 125 matched (series, diameter) pairs:

| statistic over 125 Onsrud hardwood/softwood chipload pairs | value |
|---|---:|
| minimum ratio | 0.571 |
| 25th percentile | 0.826 |
| **median** | **0.944** |
| 75th percentile | 1.000 |
| maximum | 2.333 (series 40-000, inverted) |
| **geometric mean** | **0.930** |
| implied exponent at Janka 500 → 1290 | **q ≈ 0.08** |

Cross-vendor pairs inside the LUT (17 rows matched on source, subfamily, diameter,
flutes and role) give ratios 0.613–1.000, implying **q ≈ 0.0–0.55, median ≈ 0.30**.
Four V-groove/engraving families publish the *same* chipload for hardwood and
softwood — `q = 0` exactly.

So the recommendation sits **above** the vendors' own derating and **inside** the
FPL-derived bracket. That is deliberate and it is the honest position:

- The vendors under-derate relative to constant force because in wood routing the
  binding constraint at these chiploads is surface quality and burning, not tool
  force; they compensate elsewhere (series selection, RPM, DOC).
- The crate uses this exponent to **transfer a row across materials it was not
  measured on** — including Ipe at Janka 3510, which is 2.7× beyond any chart's
  "hard wood". A transfer law should be more conservative than an in-domain chart.
- 0.5 is the value that changes one implementation instead of two: it reproduces
  the shipped formula path to **≤ 0.71 %** across the whole calibrated band.

**Uncertainty, stated.** The FPL bracket [0.48, 0.67] rests on a constant-force
assumption that no source states for routing; the vendor evidence says 0.08–0.30.
The two disagree by a factor of ~2–6 on the log axis and 0.5 is chosen *between*
them, closer to the physics. Anyone who later measures permissible chipload versus
Janka on a bench should expect to move this number, and this document is where
they should record it.

**Effect on the shipped values.**

Formula path — effectively unchanged:

| Janka | shipped composite `^0.504` | proposed `^0.5` | Δ |
|---:|---:|---:|---:|
| 300 | 1.4181 | 1.4142 | −0.28 % |
| 600 (anchor) | 1.0000 | 1.0000 | 0.00 % |
| 1450 (hard maple) | 0.6410 | 0.6433 | +0.35 % |
| 3510 (Ipe) | 0.4105 | 0.4134 | **+0.71 %** |

LUT path — this is where the change lands (`h_scale = row_Janka / query_Janka`):

| `h_scale` | scenario | today (`^1.0`) | proposed (`^0.5`) | band moves by |
|---:|---|---:|---:|---:|
| 0.3675 | oak-anchored row → Ipe query | 0.3675 | 0.6062 | **×1.65** |
| 0.50 | → 2× harder | 0.500 | 0.707 | ×1.41 |
| 0.80 | → 1.25× harder | 0.800 | 0.894 | ×1.12 |
| 1.00 | exact match (live B3) | 1.000 | 1.000 | ×1.00 |
| 1.50 | hardwood row → softer query | 1.500 | 1.225 | ×0.82 |
| 2.4167 | 1450-row → 600 softwood query | 2.417 | 1.555 | **×0.64** |

Note the change is **conservative** (band drops) when a hardwood row is applied to
a softwood query — the common case for the 122 Amana rows, most of which are
hardwood-anchored — and **permissive** when transferring into harder material.

**Sentry impact: `_litmatrix_ipe_janka_scaling` needs no re-pin.** Its only
assertion is directional (`ipe_fpt < oak_fpt`,
`crates/rs_cam_core/tests/_litmatrix_ipe_janka_scaling.rs:76`). Under `^0.5` the
Ipe scale is `0.3675^0.5 = 0.606 < 1.0`, so the direction holds. The docstring's
narrative *"derate Ipe … by roughly 1290 / 3510 ≈ 0.37×"* becomes stale and must
be corrected in the same commit — that sentence is the exact "stale rationale
outliving the code" class the programme's P11 names. **Contrast with the census's
T3.4 evidence column, which listed an `_litmatrix_ipe_janka_scaling` re-pin as
required: it is not.**

`family_default_janka` (`vendor_lookup.rs:291-304`, T4.6) is **unaffected** by this
recommendation — the anchors themselves (SPF 500, red oak 1290, MDF 700 …) are not
exponents. Its provenance-silence remains an open Checkpoint item.

---

## 5. Proposed `CREDITS.md` addition block

**Do not apply this now.** Per the Checkpoint B ruling it lands with the code that
implements §4. Reproduced here so the implementation wave does not re-derive it.

```markdown
### Chipload column convention and scaling laws

The vendor `chipload_*_mm_tooth` columns in
`crates/rs_cam_core/data/vendor_lut/observations/` are **linear advance per
tooth** — `feed_rate ÷ (spindle_rpm × cutting_edges)` — not chip thickness.
Verified 2026-08-04 against the publishers' own definitions (see
`planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` for verbatim
quotations, access dates and the per-family confidence table):

- **LMT Onsrud**, *Hard Wood* / *Soft Wood Cutting Data Recommendations* —
  <https://www.onsrud.com/images/Hard%20Wood.pdf>,
  <https://www.onsrud.com/images/Soft%20Wood.pdf> (accessed 2026-08-04).
  Column: "Recommended Chip Load per Tooth by Cutting Diameter". Prints
  "Chip Load = Feed Rate / (RPM x # of cutting edges)".
- **Freud**, *Router Bit Feed Rates and Speeds for CNC* (2017-08-22) —
  <https://www.freudtools.com/public/assets/freud/downloadables/freudtools-router-bit-feed-and-speed-for-cnc-20170822.pdf>
  (accessed 2026-08-04). Prints the same identity plus a worked example.
- **Amana Tool**, *ZrN 2D/3D Carving Feed and Chip Load Chart* and siblings —
  column "Chip Load Per Tooth (Based on 18,000 RPM)"; prints
  "To find Chip Load = IPM / (RPM x # of Flutes)". The charts publish IPM and
  chip load side by side at fixed RPM, so the identity is self-verifying to the
  chart's printed precision. Retrieved 2026-08-04 via the ToolsToday mirror
  <https://toolstoday.com/content/ProductFile/Attachments/ZrN-3D-Profiling-Feed-Chip-Load-Chart.pdf>
  (amanatool.com returns HTTP 403 to automated fetch).
- **Garr Tool**, *Chip Thinning* —
  <https://www.garrtool.com/knowledge-base/chip-thinning/> (accessed 2026-08-04).
  Establishes that the published chip load is the **programmed** feed per tooth
  and equals the maximum chip thickness only at radial engagement >= 50 % of
  diameter.

The axial derate applied by `feeds::geometry::doc_derating_scale`
(1.00 / 0.75 / 0.50 at DOC = 1 / 2 / 3 x D) is published verbatim and identically
by all three wood vendors above.

Scaling exponents in `feeds::vendor_lookup` and `machine::ChipLoadFormula` are
**regressions over those same vendor charts**, not vendor-published constants:

- diameter, `chipload ∝ D^0.61` — per-family fitted exponents span 0.23–1.25
  (Onsrud 50 series-fits median 0.37; Amana Spektra over a 16x diameter span
  0.586–0.619; Freud 1.05–1.25); row-count-weighted central estimate 0.52.
- hardness, `chipload ∝ Janka^-0.5` — bracketed by USDA Forest Service,
  *Wood Handbook*, GTR-190 (2010) Ch. 5 Table 5–11a, which gives side hardness
  ∝ G^1.49 (softwoods) / G^2.09 (hardwoods) against compression- and
  shear-parallel ∝ G^0.85–1.13; constant force per tooth implies
  Janka^-0.48…-0.67. Onsrud's own hardwood/softwood chart pairs (125 matched
  cells) derate less than this, at a geometric-mean ratio of 0.930 (implied
  exponent ≈ 0.08), so 0.5 is deliberately conservative for cross-material row
  transfer. <https://www.precisebits.com/PDF/USFS_mechanical_properties_of_wood.pdf>
  (accessed 2026-08-04).

Neither exponent is a physical constant and neither should be cited as one.
```

The block also closes FEEDS_CENSUS §7 gaps 2 and 3. Gap 5 (`onsrud_drill` /
`vectric_drill_default` have no `CREDITS.md` lineage) is **not** closed by it and
remains with the L1/documentation lane.

---

## 6. Implications ledger

### 6.1 Comparisons that change status

| id | today | under the verdict | who must act |
|---|---|---|---|
| **F-1** | gate observed (arc-mean chip) vs band (advance) — INVALID | VALID once the gate reports `fz_eff`; the fix is **deleting** D9 + chip-geometry, not inverting them | T3.1 |
| **P-1** | biased low by `1/f_lut`, 2.4×–40.4× | bias removed; and the factor is shown to have **no vendor authority on either end** (§2.3) | T3.1 |
| **C-13 / F-5** | two models 14.5× apart | gate model becomes `nominal × feed_ratio`; `arc_fit_ratio_for_op` predicts a quantity that ceases to exist → **retire, don't re-key** (census T3.5 said "replace with `f_lut × expected_feed_ratio`" — that is now wrong) | T3.5 |
| **C-6** | `^1.0` vs `^0.61` | one law `^0.61`; LUT path adopts formula path | T3.4 |
| **C-7** | census says `^1.0` vs `^1.26` — **the second figure is wrong** | true laws are `^1.0` vs `^0.504`; one law `^0.5`; LUT path adopts formula path | T3.4 + a FEEDS_CENSUS correction |
| **C-12 / T4.2** | floor 0.025 = 2.73× band max | **1.71× band max** — reduced, **not resolved**. Still two contradicting shipped policies | T3.3, unchanged as a Checkpoint item |
| **P-15** | `ae_*_mm` unscaled while chipload is scaled | **moot on the gate path** once D9 goes, since nothing reads the `ae` band; still live for anything else consuming it | T2.9, demoted |
| optimizer retarget | multiplier `target / gate-observed` | denominator changes by `1/f_lut`; targets must be **re-derived** | must be in the flip evidence (§3.4 item 6) |
| `is_extrapolated` | on applied `total_scale` | **must move to the raw ratios** or the exponent change silently un-flags rows and converts advisories into hard trips (§4.1 ⚠) | T3.4, mandatory rider |
| `_litmatrix_rubbing_floor_clamp` | pinned on an assumed convention | assumption now evidenced; **pin stands** | none — gap 3 closes |
| `_litmatrix_ipe_janka_scaling` | census listed a re-pin as required | **no re-pin needed** (directional assert); its docstring's "≈ 0.37×" narrative goes stale and must be fixed | T3.4 |

### 6.2 The W6 drill-threshold hold

**The hold can lift, conditionally.** Its stated re-open condition
(ORCHESTRATION_LOG.md:194) is *"a primary source establishing whether a vendor
chipload column is a linear advance per tooth or a mean chip thickness."* That
source now exists, is verbatim-quoted, and is numerically self-verifying (§2.2).
The axis W6's thresholds are expressed on is **known** and will not move by
2.4×–40.4×.

Three conditions on the lift, all narrow:

1. **Drill thresholds are not on the chipload axis at all.** `drill_summaries` /
   `drill_gates` are keyed on depth-to-diameter ratio, per-peck D/d and plunge
   feed per diameter (CLAUDE.md, Step 3 PR2). None of them consumes `f_lut`,
   `mean_chip_factor` or a LUT `ae` band. The inheritance the hold feared is
   **narrower than it was written**; W6 should re-scope to it rather than wait.
2. **`DRILL_CHIPLOAD_MULTIPLIER = 2.5` (`feeds/mod.rs:834`) is the one drill
   number that does inherit**, via the formula chipload. It rides on `k0·D^p`,
   whose diameter exponent this document recommends leaving at 0.61 — so it does
   not move under §4.1 either. Its own provenance ("drill chipload bands are ~2.5×
   higher…", an audit finding, not a source) is unverified and belongs on W6's
   own list.
3. **The hardness law does reach drill**, through `feed_scale_factor` — but §4.2
   shows the formula path moves by ≤ 0.71 %, i.e. below any drill threshold's
   resolution. Drill queries that hit the **LUT** (`onsrud_drill`) will move by the
   §4.2 LUT-path table.

Recommendation to the orchestrator: **lift the W6 hold and re-scope it** to (a)
the drill-specific sources FEEDS_CENSUS §7 gap 5 names, (b)
`DRILL_CHIPLOAD_MULTIPLIER`'s provenance, and (c) whether any drill threshold
reads a LUT band at all. Do **not** lift the separate T3.1 ordering constraint —
drill recalibration still follows T3.1, but it no longer waits on T4.1, because
T4.1 is answered.

### 6.3 Findings recorded, not fixed

- **`sources.toml` cites four dead URLs.** `onsrud_hwood` and `onsrud_doc_rule`
  → `onsrud.com/files/pdf/series_70_85.pdf`, `onsrud_swood` →
  `…/series_60_softwood.pdf`, `onsrud_drill` → `…/drill_chart.pdf` — all
  **HTTP 404**, tested 2026-08-04. The live equivalents are at
  `onsrud.com/images/*.pdf` and are already in the LUT manifest under different
  ids. Two further Onsrud rows (`onsrud_plywood`, `onsrud_plastic`) share the dead
  URL family and were not individually tested. The freshness engine reports all 32
  sources `fresh` because `last_verified` is hand-set and **nothing validates
  `citation_url`** — the same shape as FEEDS_CENSUS §7 gap 6 (six `"(pending)"`
  URLs), now shown to extend to URLs that look valid and are not.
  Owner: `/refresh-lit-matrix`, next run. Re-open: immediately — this is a
  four-source citation failure in a gold tier.
- **`sources.toml`'s `fpl_wood_handbook` cites GTR-282**, not GTR-190. The
  scaling functions used in §4.2 are Table 5–11a of **GTR-190** Chapter 5, which
  is the document the LUT manifest already cites as `fpl_ch5_2010`. Two source
  records for one publication, pointing at different reports.
- **`machine.rs:38` attributes `k0/p/q` to "Shapeoko empirical data"** with no
  citation anywhere in `sources.toml` or `CREDITS.md`. §4 does not verify that
  attribution; it **replaces the justification** for `p` and `q` with the vendor
  regression and the FPL bracket, while leaving the values where they are. `k0`
  (0.024) is untouched and remains unsourced.
- **`amana_zrn_3d_profiling` contains one internally inconsistent chart cell** —
  2-flute ball nose 1/16" Wood prints 55–90 IPM and 0.003"–0.005" CPT, but
  `55/(18000·2) = 0.00153` and `90/(18000·2) = 0.0025`, exactly half. The repo
  already detected this and marked that row `row_kind: "derived"`
  (`source_manifest.json` coverage note). Confirmed independently this wave. It is
  the *only* cell in any retrieved Amana chart where the printed identity fails,
  which strengthens rather than weakens §2.2.
- **176 of 252 LUT rows carry no `ae`.** D9 is already inert for them
  (`chipload.rs:522-523`). The unit defect therefore affects a **minority** of
  rows — but the majority of *3D finishing* rows, because `ae` was authored
  exactly where a scallop stepover was needed.

---

## 7. NOT VERIFIED ledger

Recorded per `/refresh-lit-matrix` policy: what was attempted, what failed, and
what the claim rests on instead.

| id | claim | what was attempted | status |
|---|---|---|---|
| **N-1** | The convention on the nine individually-unretrieved Amana charts: Spektra Spiral Plunge v24, Ball Nose v7, Insert V-Groove v16, Spoilboard v8, Compression Spirals v8, AMS-159 V-Groove v2, Spektra Engraving v4, ZrN Aluminum O-Flute v13, ZrN 3D Profiling **v8** | `WebFetch` on the `amanatool.com/pub/media/productattachments/…` URLs in `source_manifest.json` — **HTTP 403 Forbidden** on every attempt | **VERIFIED (publisher-level), NOT VERIFIED per chart.** Three Amana charts retrieved from two independent mirrors all carry the identical footer block, and one of the three is the chart behind the live B3 row. A per-chart check would require a browser session |
| **N-2** | The values in the Onsrud rows sourced from `onsrud_cutting_data_recommendations` (the 10 `ae`-bearing ones) | The landing page <https://www.onsrud.com/Forms/Cutting-Data-Recommendations.asp> was retrieved; it carries **no** definitions or formulas, only links to per-material PDFs | **NOT VERIFIED for those 10 rows' `ae` values.** Their `ae_rule` strings ("10% to 30%D" etc.) are repo-authored (§2.3) and appear on no retrieved chart. The *convention* is verified from the Hard Wood / Soft Wood PDFs |
| **N-3** | Whiteside Fusion360 rows: that the `.tools` field is `feedPerTooth` and its value | The source URL is a Dropbox share link (`dropbox.com/s/bqw4gqeggcv30o1/…`); not fetched | **NOT VERIFIED.** Convention rests on the Fusion 360 tool-library schema as documented by Autodesk, not on the file. Rows are already `evidence_grade: "c"`, `row_kind: "fallback"` |
| **N-4** | Helical Solutions Machining Guidebook 2016 — exact wording on chip thinning and on the two HEM/traditional data points | PDF at `web.mae.ufl.edu/…/Helical_Machining_Guidebook.pdf` not retrieved; only a search-engine summary was obtained | **NOT VERIFIED (wording).** Direction corroborated by Garr (retrieved) and Harvey Performance (search summary). 2 rows affected |
| **N-5** | Garr Tool `TECH_MILLING_ALUMINUM.pdf` column headings | Not fetched; the Garr chip-thinning KB page was fetched instead | **NOT VERIFIED (column heading).** The convention is verified from the same publisher's chip-thinning page; the per-row `ae_rule` transcriptions are repo-internal |
| **N-6** | That Sandvik Coromant publishes an *average* chip thickness `h_m` distinct from `f_z` | <https://www.sandvik.coromant.com/en-us/knowledge/milling/entering-angle-and-chip-thickness> retrieved; it defines `h_ex` (maximum) and states *"With 90 degree cutters the feed per tooth equals the maximum chip thickness (f_z = h_ex)"*, and gives an ae/Dc feed-modification table rising to 2.3× at 5 % engagement. **`h_m` does not appear on that page** | **PARTIALLY VERIFIED.** The `f_z ≠ chip thickness` distinction and the "increase feed at low ae" direction are verified. The specific `h_m` term is not, and this document does not rely on it |
| **N-7** | The "Shapeoko empirical data" provenance of `k0 = 0.024`, `p = 0.61`, `q = 1.26` (`machine.rs:35-42`) | Searched `sources.toml`, `CREDITS.md` and the repo for a Shapeoko citation; `sources.toml` carries `shapeoko_wiki` but cited for *materials*, not for these coefficients. Not fetched | **NOT VERIFIED.** §4 does not rely on it — it supplies an independent justification for `p` and `q`. `k0` remains unsourced |
| **N-8** | That the vendor charts' chipload columns were measured at any particular *radial* engagement | Six wood charts read in full; none states one | **VERIFIED ABSENT** for wood (a negative result, deliberately recorded as one). **VERIFIED PRESENT** for the two metal families, both at ≥ 0.5 D |
| **N-9** | Whether the ratios in §4.2's Onsrud comparison reflect chipload policy alone, or are confounded by RPM/SFM differences between the two charts | Neither chart prints RPM per series (RPM is fixed by series); no RPM column exists to compare | **NOT VERIFIED (confound not excluded).** The comparison is same-series, same-diameter, so tool geometry is controlled; a per-series RPM difference between the two charts cannot be ruled out from the retrieved documents |
| **N-10** | Whether any published source gives a chipload–diameter exponent directly | Searched; none found. Every figure in §4.1 is regressed by this wave from vendor tables | **NO PRIMARY SOURCE EXISTS** (as far as this wave could establish). §4.1's numbers are **derived**, and the proposed `CREDITS.md` block says so explicitly |

---

## 8. Answers, in one line each

1. **T4.1 — is the vendor chipload column an advance per tooth or a mean chip
   thickness?** Advance per tooth, in every source family in the shipped LUT;
   HIGH confidence for Onsrud, Freud, Amana, Garr and IDC, MEDIUM for Whiteside
   and Helical; no family reads as a mean chip thickness and none is ambiguous on
   its operational definition.
2. **T3.4/T4.3 — one diameter law?** `chipload ∝ D^0.61` — the LUT path adopts the
   formula path's existing exponent; regressed range across seven vendors is
   0.23–1.25 with a row-weighted centre of 0.52, and piecewise was tested and
   rejected.
3. **T3.4/T4.3 — one hardness law?** `chipload ∝ Janka^-0.5` — again the LUT path
   adopts the formula path, whose true composite exponent is **0.504, not the
   census's 1.26**; bracketed [0.48, 0.67] by FPL GTR-190 Table 5–11a under
   constant force per tooth, and deliberately more conservative than the vendors'
   own 0.08–0.30.
4. **Biggest implication for the gate conversion.** The correction is a
   **deletion**, not an inversion — and on the live B3 op it moves the operation
   from *16 % of band minimum with a burn advisory* to **99.9 % of band maximum**,
   one percent of feed away from `Exceeds(High)`.
5. **W6 drill hold.** Can lift and be re-scoped: its stated re-open condition is
   met, and the drill gates turn out not to sit on the chipload axis at all — but
   the T3.1 *ordering* constraint stands.
