# VSBS — vendor side-by-side chipload spot-check

Date: **2026-08-16**. Branch `tech-debt-3`. Base `3c506934`
(TD3 close-out). Post-closeout follow-up, operator-requested:
*put the engine's recommendation next to the exact vendor chart row it
derives from, so a machinist can see how close we sit to
industry-recommended numbers.*

**Nothing behavioural changed in this wave.** One new probe file,
`crates/rs_cam_core/tests/vendor_sidebyside_chipload.rs` (4 tests, all
green), whose every assertion is a reproduction of the current state.
Raw capture: `artifacts/vsbs/probe_run.txt`.

Vocabulary, fixed by `CHIP_THICKNESS_POLICY.md` and the 2026-08-06 unit
deletion, and used consistently below: **a vendor chipload column is an
advance per tooth**, `feed ÷ (rpm × flutes)`. That is also what the
post-sim gate observes, and what "commanded" means in every table here.
It is **not** a chip thickness.

---

## 0. Verdict, in one paragraph

Across six probes driven through `feeds::calculate` on the shipped LUT,
**three of six recommendations land inside the vendor window and they
land low in it — 7 %, 10 % and 25 % of the way up from the band floor**
(ratios to the derated band: 1.05×–1.38× the minimum, 0.55×–0.63× the
maximum). That is a conservative, floor-hugging engine, and against a
vendor's own published range it is defensible. The other three do not
land inside, and each misses for its own reason: **B** (Ø6 profile in
oak) sits at **0.56× the band floor** *only because the machine's
4 000 mm/min feed ceiling truncated it* — unclamped it would have been
**1.61× the band maximum**; **E** (Ø12.7 pocket in oak) sits at
**0.79×** a single-value Amana chart cell that has no window at all;
and **D** (Ø1.5 ball finishing pine) sits at **1.48× the band maximum
as shipped, 2.17× unclamped**, because chip-thinning compensation
multiplies the seed by 3.59 and the gate's yardstick is engagement-blind.
So the honest one-line answer is: *where the recipe is not being pushed
by chip-thinning or pulled by the machine ceiling, we sit in the bottom
quarter of the vendor's own range; where it is, we can leave the range
in either direction, and nothing in the pre-simulation path notices.*

---

## 1. What the engine actually does, measured

Two claims in circulation needed checking against code, and both need
correcting.

### 1.1 "Suggest rests at 0.999× the derated band minimum" — that is an outcome on two fixtures, not the mechanism

The Checkpoint J-1 retirement comment
(`crates/rs_cam_core/src/feeds/suggest.rs`, the pass-8 tombstone) says
removing the arc-fit lift *"leaves the commanded advance per tooth at
the derated band **minimum**, which the calculator already reaches
unaided (measured **0.999×** on both Adaptive3d fixtures)"*. That
reading is true of those two fixtures. It is not a target, and it does
not generalise. Measured here on six probes, the commanded advance sits
at **0.56× / 0.79× / 1.05× / 1.07× / 1.38× / 2.42×** the derated band
minimum.

The mechanism, pinned by
`the_recommendation_is_the_transferred_band_midpoint_times_the_derate_stack`:

```text
commanded fpt = target_chip_load_mm            (matched row MIDPOINT, transferred)
              x combined_chip_thinning         (radial RCTF x axial ball thinning, clamped [1,4])
              x depth_tier                     (1.00 / 0.75 / 0.50 / 0.45 step)
              x ld_overhang x workholding
              x power_limit x feed_clamp
              x safety_factor                  (0.75 on generic_wood_router)
              x spindle_speedup                (1.0 under MatchChart)
```

exact to float precision on every unclamped, unlimited probe. The seed
is the band **midpoint** (`vendor_lookup::build_result` —
`chipload_midpoint(obs) * total_scale`), not the minimum. Landing near
the floor is what happens when the multipliers compose near
`band_min / band_mid`; on four of six probes here they compose to
**0.7862**, which is exactly `safety 0.75 × RCTF(0.35 D) 1.048285` — the
default pocket/adaptive stepover, nothing more.

Corollary worth stating plainly: **`SuggestAggressiveness` is inert on
the pre-simulation path.** `SuggestAggressiveness::target_chipload` has
zero production call sites since pass 8 was retired (grep over
`crates/*/src`); the code says so at `suggest.rs:4600-4608`. Setting
Conservative / Default / Speed does not move the feed.

### 1.2 The two sides of the comparison carry DIFFERENT DOC derates

Pinned by `the_band_derate_and_the_feed_derate_are_different_functions`.

| side | function | 1×D | 1.5×D | 2×D | 2.5×D | 3×D | 4×D |
|---|---|---:|---:|---:|---:|---:|---:|
| the **band** the gate judges against (`chipload_bounds`) | `geometry::doc_derating_scale` — piecewise **linear** | 1.000 | **0.875** | 0.750 | **0.625** | 0.500 | 0.500 |
| the **feed** the recommendation is built from (`derates.depth_tier`) | `geometry::depth_tier_multiplier` — **step** | 1.000 | **0.750** | 0.750 | **0.500** | 0.500 | 0.450 |

They agree exactly at the three break points the vendor rule is quoted
from (Onsrud / Freud / Amana all print *"1×D use recommended chip load;
2×D reduce 25 %; 3×D reduce 50 %"*) and diverge everywhere between them.
At DOC/Ø = 1.5 the band is derated 0.875 while the feed is derated
0.750, so the same recipe reads **14 % lower against the band** than it
would at 2×D — a purely bookkeeping movement of the verdict. This is
adjacent to, but not the same as, the already-ledgered DOC-derate
denominator divergence (census F-3 / C-2 / C-5, on
`FeedsResult::effective_diameter`).

---

## 2. The six rows, side by side

Common to every row: machine `MachineProfile::generic_wood_router()`
(4 000 mm/min max feed, safety factor 0.75), `SpindleStrategy::MatchChart`,
2 flutes, default stepover, `axial_depth_mm` set explicitly to the stated
DOC. Every number below is printed by production code
(`artifacts/vsbs/probe_run.txt`), not re-derived in prose.

### 2.1 The table

| | probe | matched row | (a) VENDOR PUBLISHED, verbatim | (b) engine's STORED band (mm/tooth) | transfer applied | (c) DERATED band at the stated DOC | (d) engine's commanded advance | (e) (d) ÷ derated [min, mid, max] |
|---|---|---|---|---|---|---|---|---|
| **A** | Ø3.0 flat, **Ipe** (Janka 3510), pocket rough, **DOC 0.6** (0.20×D) | `freud-solid-carbide-eighth-hardwood` | Freud, *Router Bit Feed Rates and Speeds for CNC* (2017-08-22) p. 2, "CHIP LOADS FOR FREUD SOLID CARBIDE ROUTER BITS ONLY", 1/8″ Hardwood: **`.002″–.005″`** | 0.0508 – 0.1270 | Ø ×0.966007, Janka ×0.606235 → **×0.585628**; DOC derate 1.000 | **0.029750 – 0.074375** | **0.040932** | **1.376× / 0.786× / 0.550×** — INSIDE, 25.1 % up the band |
| **B** | Ø6.0 flat, white oak (1360), contour **finish**, **DOC 12.0** (2.0×D) | `onsrud-hardwood-60-100mw-1_4-finish` | LMT Onsrud, *Hard Wood Cutting Data Recommendations* p. 115, series **60-100MW**, 1/4″ column: **`.014–.016`** in/tooth | 0.3556 – 0.4064 | Ø ×0.966007, Janka ×0.973925 → ×0.940818; DOC derate **0.750** | **0.250916 – 0.286761** | **0.141372** | **0.563× / 0.526× / 0.493×** — BELOW the floor, *and only because the 4 000 mm/min machine ceiling cut it*; unclamped it is **1.844× / 1.721× / 1.613×** |
| **C** | Ø6.0 flat, hard maple (1450), pocket rough, DOC 4.0 (0.67×D) | `amana-flat-hardwood-pocket-6000-2f` | ⚠ **no verbatim chart cell** — see §3.2. Cites the Amana Spektra Spiral Plunge v24 chart, which per its own manifest note *"does NOT separate softwood from hardwood"* | 0.0320 – 0.0550 | Ø ×1.0, Janka ×1.0 → ×1.0; DOC derate 1.000 | **0.032000 – 0.055000** | **0.034200** | **1.069× / 0.786× / 0.622×** — INSIDE, 9.6 % up the band |
| **D** | **Ø1.5 ball** (sub-Ø2), radiata pine (710), parallel **finish**, DOC 0.3 | `amana-ball-softwood-parallel-1587-2f-zrn` | Amana, *ZrN 3D Profiling Feed & Chip Load Chart*, p. 1, 2-Flute Ball Nose, **1/16″ column**, Wood/MDF/Sign-Foam row. ⚠ The chart's **printed** CPT is `0.003″–0.005″`; the repo **rejected it** as inconsistent with the same cell's printed 55–90 IPM at 18 000 rpm 2F and stored the IPM-derived **`.00153″–.0025″`** instead | 0.0388 – 0.0635 | Ø ×0.966007, Janka ×0.919277 → ×0.888028; DOC derate 1.000 | **0.034455 – 0.056390** | **0.083333** | **2.419× / 1.835× / 1.478×** — **ABOVE the ceiling**, and that is *after* the machine clamp; unclamped **3.552× / 2.694× / 2.170×** |
| **E** | Ø12.7 flat, white oak (1360), pocket rough, DOC 6.0 (0.47×D) | `amana-compression-wood-pocket-12700-2f` | Amana, *Solid Carbide Compression Spiral Router Bits v8* p. 1, 1/2″ row, Wood column: **280 IPM / `.0077″`** — a **single value, no range** | 0.1956 – 0.1956 (point preset) | Ø ×1.0, Janka ×0.973925 → ×0.973925; DOC derate 1.000 | **0.190500 – 0.190500** | **0.149773** | **0.786× / 0.786× / 0.786×** — 21 % BELOW the only number the chart publishes |
| **F** | Ø3.0 flat, **6061-T6 aluminium**, pocket rough, DOC 1.5 (0.5×D) | `amana-zrn-flat-aluminum-pocket-3175-2f` | Amana, *ZrN 3D Profiling Feed & Chip Load Chart*, p. 1, 2-Flute Flat Bottom, 1/8″ column, Aluminum/Copper/Brass row: **`.003″–.005″`** | 0.0762 – 0.1270 | Ø ×0.966007, Janka ×1.0 (Brinell row, no Janka anchor) → ×0.966007; DOC derate 1.000 | **0.073610 – 0.122683** | **0.077164** | **1.048× / 0.786× / 0.629×** — INSIDE, 7.2 % up the band |

Vendor coverage: **1 Freud (grade a), 1 Onsrud (grade b, OCR/pdftotext),
4 Amana (grade a).** Garr was probed and not selected — its aluminium
rows lost the Ø3 match to `amana-zrn-flat-aluminum-pocket-3175-2f`, which
is itself a useful datum: the matcher's choice, not the analyst's, decides
which vendor a verdict is measured against.

### 2.2 Inch-conversion audit of column (a) vs column (b)

Every stored bound that claims to be a chart cell was re-multiplied:

| stored | claimed cell | `inch × 25.4` | agrees? |
|---|---|---|---|
| 0.0508 / 0.1270 | Freud `.002″` / `.005″` | 0.05080 / 0.12700 | **exact** |
| 0.3556 / 0.4064 | Onsrud `.014″` / `.016″` | 0.35560 / 0.40640 | **exact** |
| 0.0388 / 0.0635 | Amana ZrN IPM-derived `.00153″` / `.0025″` | 0.038862 / 0.06350 | 0.0388 is the 3-s.f. rounding of 0.038862 (−0.16 %) |
| 0.1956 | Amana compression `.0077″` | 0.195580 | 4-d.p. rounding, +0.01 % |
| 0.0762 / 0.1270 | Amana ZrN `.003″` / `.005″` | 0.07620 / 0.12700 | **exact** |
| 0.0320 / 0.0550 | — | — | **no inch preimage** (§3.2) |

---

## 3. Two rows worked digit for digit

### 3.1 Probe A — the Ipe row, reproducing S-2 §5.3 exactly

Freud publishes, verbatim (retrieved and text-extracted 2026-08-13,
`LIT_MATRIX_REFRESH_S2.md` §5.3):

```text
  Tool                                         Hardwood   Softwood
Diameter
   1/8"                                      .002"-.005" .004"-.006"
```

`.002″ × 25.4 = 0.0508` and `.005″ × 25.4 = 0.1270` — the stored row to
the digit.

The query is a **Ø3.0 mm** tool in **Ipe (Janka 3510)** against a
**Ø3.175 mm** row that carries **no per-row hardness**, so
`family_default_janka(Hardwood) = 1290` supplies the anchor
(`vendor_lookup.rs:479-492`).

```text
raw diameter ratio  = 3.0 / 3.175                  = 0.944882
raw hardness ratio  = 1290 / 3510                  = 0.367521
diameter scale      = 0.944882 ^ 0.61              = 0.966007      (CHIPLOAD_DIAMETER_EXPONENT)
hardness scale      = 0.367521 ^ 0.50              = 0.606235      (CHIPLOAD_HARDNESS_EXPONENT)
total scale         = 0.966007 x 0.606235          = 0.585628

band min = 0.0508 x 0.966007 x 0.606235            = 0.029750
band max = 0.1270 x 0.966007 x 0.606235            = 0.074375
```

Both reproduce S-2's `0.0508 × 0.6062 × 0.9660 = 0.0297` exactly.
`|ln(0.944882 × 0.367521)| = 1.0577 > ln 1.4 = 0.3365`, so the row is
flagged **`is_extrapolated = true`** — the verdict is `Approximate` and,
per `low_side_is_advisory()`, its burn side is advisory only.

DOC 0.6 on Ø3 gives DOC/Ø = 0.20 ≤ 1.0, so `doc_derating_scale` is
1.000 and the derated band equals the transferred one.

The recommendation:

```text
seed chipload  = (0.029750 + 0.074375) / 2         = 0.052062   (band MIDPOINT)
RCTF           = 3.0 / (2 sqrt(1.05 x (3.0-1.05))) = 1.048285   (ae = 0.35 D)
axial thinning = 1.0 (flat tool)
depth_tier     = 1.000  (DOC/O = 0.20)
safety         = 0.750
commanded fpt  = 0.052062 x 1.048285 x 1.000 x 0.750 = 0.040932 mm/tooth
feed           = 0.040932 x 21220.7 rpm x 2 flutes   = 1737.2 mm/min
```

Position in the vendor window: `(0.040932 − 0.029750) / (0.074375 −
0.029750) = 25.1 %`. Against Freud's **unscaled** published band it is
`0.040932 / 0.0508 = 0.806×` the floor — i.e. below the chart, which is
the Janka transfer doing its job on a wood 2.7× harder than anything
Freud's "Hardwood" column was measured on.

### 3.2 Probe C — the fixture row the whole programme has been quoting, and its provenance hole

`amana-flat-hardwood-pocket-6000-2f` is the most-cited row in TD3: it is
the matched row on the A-8 retarget fixture, the A-5i arc-fit fixture, and
the `_litmatrix_rubbing_floor_clamp` cells. Its arithmetic is trivial —
Ø6.0 query against a Ø6.0 row, Janka 1450 query against a Janka 1450 row:

```text
raw diameter ratio = 6.0 / 6.0     = 1.000000  ->  scale 1.000000 ^0.61 = 1.000000
raw hardness ratio = 1450 / 1450   = 1.000000  ->  scale 1.000000 ^0.50 = 1.000000
derated band       = 0.032000 .. 0.055000     (DOC/O = 0.667, derate 1.000)

seed        = (0.032000 + 0.055000)/2                = 0.043500
RCTF        = 6.0 / (2 sqrt(2.1 x (6.0-2.1)))        = 1.048285
commanded   = 0.043500 x 1.048285 x 1.000 x 0.750    = 0.034200 mm/tooth
feed        = 0.034200 x 15000 rpm x 2 flutes        = 1026.0 mm/min
```

9.6 % up the band. **What has no arithmetic is column (a).** `0.032` and
`0.055` have no inch preimage: `0.032 / 25.4 = .00126″` and
`0.055 / 25.4 = .00217″`, neither of which is a chart cell. The row
carries `evidence_grade: "a"` and `row_kind: "exact"` and cites
`amana_spektra_spiral_plunge_v24`, but:

- it has **no entry in any provenance document** —
  `planning/data_ingest_2026-05-29/amana_provenance.md` covers five Amana
  charts and this row is in none of them;
- `git log -S` puts it in **`def88067` (2026-03-21)**, the original
  "Add vendor LUT chipload seeding" commit, i.e. two months before the
  evidence-graded ingest rounds that produced every verbatim row above;
- the manifest's own coverage note for that source says the chart
  *"does NOT separate softwood from hardwood"* — yet this row is tagged
  `material_family: hardwood` with `hardness_value: 1450`;
- its `ap_rule` (`"0.25xD to 0.7xD"`) and `ae_rule` (`"17% to 37%D"`) are
  repo-authored application windows, the class S-2 §3.2 already
  identified as *"a stepover recommendation, not a vendor measurement
  condition"*;
- and `amana_spektra` is one of the **four URLs S-2 could not re-verify**
  (403 to curl *and* to a browser-shaped fetcher, ledgered **DR-403**),
  so nobody has read the chart since 2026-03.

The whole file `observations/amana_flat_end.json` (20 rows) is from that
same seeding commit. **This is not a claim that the numbers are wrong** —
they are plausible and they have never misbehaved. It is a claim that
grade `a` / `exact` is unearned on them, and that a document titled
"side by side with the vendor chart" cannot put a chart cell in
column (a) for this row. Ledger this as **VSBS-SEED**.

---

## 4. Where the recommendation actually sits, summarised

| probe | inside the derated window? | position | why not, if not |
|---|---|---|---|
| A Ipe Ø3 pocket | **yes** | 25.1 % up | — |
| B oak Ø6 profile | no, **below** | 0.56× floor | machine feed ceiling 4 000 mm/min; unclamped it is **1.61× the ceiling** |
| C maple Ø6 pocket | **yes** | 9.6 % up | — |
| D pine Ø1.5 ball finish | no, **above** | 1.48× ceiling | chip-thinning stack ×3.59 (RCTF 2.874 × axial 1.250); unclamped **2.17×** |
| E oak Ø12.7 pocket | no, **below** | 0.79× | the chart cell is a single value — there is no window |
| F 6061 Ø3 pocket | **yes** | 7.2 % up | — |

Read the two "no, below" rows carefully before treating them as
conservatism. **B is not conservative** — it is a recipe that wanted to
run 61 % *above* the vendor ceiling and was rescued by a machine limit;
put the same job on a faster gantry and it leaves the window upward.
**E** is measured against a point preset, where the low side is
`ChipBoundsSource::VendorLutPointPreset` and therefore **advisory only**
(`verdict.rs:656-671`) — the gate would not refuse it, and the optimizer's
`ChiploadFeedRetargeter` would not act on it either.

---

## 5. Gaps

### 5.1 Both transfer laws are repo-derived; no primary source publishes either

`CREDITS.md` says so in those words:

- **`chipload ∝ D^0.61` — "derived here."** Per-family fitted exponents
  over the shipped charts span **0.23–1.25** (Onsrud median 0.37; Amana
  Spektra 0.586–0.619 over a 16× diameter span; Freud 1.05–1.25);
  row-weighted centre 0.52, unweighted cross-family median 0.40. 0.61 was
  chosen because it is already `machine::ChipLoadFormula::default().p`,
  so only one implementation moves. **It is a fitted summary of seven
  vendors who disagree, not a physical constant**
  (`vendor_lookup.rs:352-386`).
- **`chipload ∝ Janka^-0.5` — "derived here, bracketed by a primary
  source."** FPL GTR-190 (2010) Ch. 5 Table 5–11a brackets it
  `[0.48, 0.67]`; 0.5 is the conservative edge. The vendors' own charts
  derate **much less**: 125 matched Onsrud hardwood/softwood pairs imply
  `q ≈ 0.08`, and four V-groove families publish `q = 0` exactly.

Both laws are load-bearing on four of the six rows above. On probe A
they move Freud's published band by **×0.585628** — a **41 % reduction
that no vendor published**, and the single largest term between the
chart cell and the number the operator is handed. Neither law may be
cited to a vendor.

### 5.2 The engine is conservative against vendor mid-band, and the lift is delegated to simulation — but the "+20.1 %" is one fixture of four

Where the recommendation lands inside the window it lands at 7–25 % up
from the floor, against a vendor range whose midpoint is the natural
target. Under Checkpoint J the intended remedy is
`adaptive_feed_modulation` (default-**ON** since J-3), which corrects
from a measured observation rather than a family constant.

**The number in circulation needs qualifying.** `+20.1 %` is A-5's
measured *median Δfeed on arm B* for **fixture A3D-1 only**
(`ARC_FIT_RATIO_EVIDENCE.md:200`). The full table is:

| fixture | moves touched | median Δfeed, arm A | median Δfeed, **arm B** |
|---|---|---|---|
| A3D-1 | 261 / 271 | −78.9 % | **+20.1 %** |
| A3D-2 | 184 / 212 | −68.5 % | **−15.7 %** |
| DC-1 | 7395 / 7395 | −80.0 % | **−40.8 %** |
| DC-2 | 14640 / 14640 | −74.6 % | **−38.4 %** |

So on 3 of 4 fixtures the simulation-backed modulator **lowers** the
retired-lift recommendation rather than raising it, by 16–41 %. Quoting
`+20.1 %` as "the lift where safe" without the other three rows
overstates the headroom by a wide margin. What the table *does*
establish, and what matters more, is that arms commanded **2.4×–5.7×
apart converge on the same gate observation** — modulation is the
arbiter, and the pre-simulation number is a starting point, not a
target.

Not measured here: how much headroom modulation finds on any of the six
probes above. That needs a simulated project, not a `feeds::calculate`
call.

### 5.3 Sub-Ø2 verdicts are provisional, and probe D shows why

Probe D is the sub-Ø2 case and it is the worst-behaved of the six.

- **Diameter transfer is doing real work at a scale nobody benched.**
  `LAW_MAGNITUDE_TABLES.md` §2 measured the `D^0.61` law's median at
  **×2.056 at Ø1** against a Ø6.35 pivot, with extremes ×2.455, *all
  clamp-bound and all already flagged extrapolated*. Probe D's own
  transfer is mild (×0.966) only because the LUT happens to carry a
  Ø1.5875 ball row; a Ø0.8 or Ø2.4 query against the same row would be
  in the ×2 regime.
- **The stored band is itself a repo recomputation, not a chart cell.**
  The Amana ZrN chart's printed CPT for this cell is `0.003″–0.005″`
  (0.0762–0.1270 mm); the repo rejected it as arithmetically inconsistent
  with the same cell's printed 55–90 IPM at 18 000 rpm 2-flute and stored
  the IPM-derived `.00153″–.0025″` instead. That is defensible — and it
  means the "vendor band" this recommendation is judged against is
  **2.0× tighter than what the vendor printed**. Carry the printed cell
  through the same transfer (×0.888028) and the band becomes
  0.067668 – 0.112780; probe D's commanded 0.083333 is then **0.739× the
  ceiling — comfortably inside** (and even unclamped, 1.085×, only just
  outside). **The verdict flips on which of the vendor's own two mutually
  inconsistent numbers you believe**, and the repo's choice is the
  conservative one.
- **The chip-thinning stack is engagement-aware and the gate is not.**
  Combined thinning is **3.592** here (RCTF 2.874 at ae = 0.0375 mm,
  axial ball thinning 1.250 at DOC 0.3 on Ø1.5). The engine raises the
  *advance* so the *chip* stays at the vendor value. Since 2026-08-06 the
  gate compares the raw advance to the vendor band with **no engagement
  awareness at all** (`CHIP_THICKNESS_POLICY.md` §0), so it reads
  `Exceeds(High)` on a recipe whose intended chip is inside the window.
  This is the same shape A-5 measured on 4/4 fixtures ("Suggest's own
  recommendation fails the gate Suggest claims to target") and is the
  live subject of **Checkpoint Q1** (parked to TD4 by the 2026-08-16
  intake triage).

Do not read a sub-Ø2 chipload verdict as a physical statement until
those three are separated.

### 5.4 Smaller gaps, named

- **Column (a) does not exist for the whole `amana_flat_end.json` file**
  (20 rows, seeded 2026-03-21, no provenance document, grade `a` /
  `exact` unearned). **VSBS-SEED**, §3.2.
- **Onsrud rows are `evidence_grade: b`** — `pdftotext -layout` extraction
  cross-checked by tesseract OCR on spot rows, not a hand-verified chart.
  The 60-100MW 1/4″ cell used above is quoted from
  `planning/data_ingest_2026-05-30/onsrud_ocr_provenance.md`, which
  records `.014-.016` for it.
- **`amana_spektra`, `amana_vbit`, `plastic_machining_guide`, `woodweb`
  remain 403-blocked** (S-2 **DR-403**); `amana_spektra` alone is cited
  142 times. Two of the six rows here descend from unverifiable-since-2026-03
  Amana sources.
- **The band derate and the feed derate are different functions** (§1.2).
  Agreeing at 1×/2×/3×D and diverging between them is not obviously wrong,
  but it is undocumented and it moves verdicts.
- **`SuggestAggressiveness` is inert** (§1.1). A user-visible dial with no
  production consumer.
- **NOT MEASURED here**: whether any of these six recommendations survives
  simulation. Every number in this document is pre-simulation. The sim
  remains the operational arbiter, and `run_simulation` was not run.

---

## 6. Reproduction

```bash
cargo test -p rs_cam_core --test vendor_sidebyside_chipload -- --nocapture --test-threads=1
```

4 tests, all green 2026-08-16 (`artifacts/vsbs/probe_run.txt`).
`cargo clippy -p rs_cam_core --test vendor_sidebyside_chipload -- -D warnings`
clean; `rustfmt --check` clean.

| test | pins | goes red when |
|---|---|---|
| `vendor_sidebyside_chipload_spotcheck` | all six probes resolve a usable recipe; ≥ 5 of 6 resolve a banded vendor row. Prints every number in §2 | a probe stops matching a banded row, or produces a zero recipe |
| `the_recommendation_is_the_transferred_band_midpoint_times_the_derate_stack` | the §1.1 identity, to 1e-9, on every unclamped vendor-banded probe | any pre-simulation pass starts moving the feed again, or the seed stops being the row midpoint |
| `the_band_derate_and_the_feed_derate_are_different_functions` | probe B's seed is the **un-DOC-derated** midpoint while its published band is derated 0.75 | the two derates are unified (which would be a fix, and should re-pin this deliberately) |
| `the_sub_two_millimetre_ball_probe_commands_above_its_own_vendor_band` | probe D's commanded advance exceeds its own derated band maximum | the chip-thinning stack or the ball-finishing defaults move |
