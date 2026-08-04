# Drill gate / evidence audit — W6 (R5), 2026-08-04

Research deliverable for `TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md` §"M2 / R5",
executed under the re-scope B-lit issued in `CHIPLOAD_LITERATURE_VERDICT.md`
§6.2 (hold lifted; four additions in scope).

**Research only.** No production file was modified. No threshold, severity,
band or default moved. Every recommendation below is a *proposal* for
Checkpoint D.

**Ordering constraint carried forward:** T3.1 (the gate-side unit conversion)
sequences before any drill *recalibration*. §10 states, per item, whether that
dependency is real. Short answer: it is real for **none of the three drill
gates** — but it *is* real for `DRILL_CHIPLOAD_MULTIPLIER`, which is not a
gate.

---

## 0. Answers first

| Question | Answer |
|---|---|
| Does **any** drill threshold read a LUT band? | **No — and no drill query can reach the LUT at all.** 0 of 256 bundled LUT rows carry `operation_family: "drill"`, and `vendor_lookup::passes_must_match` hard-filters on exact family equality. Every drill number is a hardcoded material constant or the `k0·D^p·(1/H)^q × 2.5` formula. §4 |
| `DRILL_CHIPLOAD_MULTIPLIER = 2.5` provenance | **Unsourced, and its stated justification is arithmetically false.** The comment's own premise ("milling-formula chipload lands ~0.03 mm/rev") is off by 3–7×; the observation it names (0.026 implied on Wanaka) was the *plunge-baseline clobber* that Step 9c independently fixed. Against the only retrievable primary wood-drill chart the factor is **under**-sized by ≈2×, not over. §5 |
| Does any verdict contradict its displayed evidence? | **Yes — four, all shipped, all operator-facing.** The headline: a `Caution` diagnostic whose message is literally `"Chip welding (D/d) exceeds: 7.33 vs 8.00"`. §7 |
| `onsrud_drill` lineage | URL 404; no `CREDITS.md` entry; the live replacement (`onsrud.com/images/Drill.pdf`, retrieved) **contains none of the data cited to it** — no peck guidance, no depth/diameter guidance, and a chip-load band that does not overlap the band the matrix cites to it. §6 |
| `citation_url` validation gap | Wider than reported: **10 of 32 source rows resolve to hard HTTP 404**, one more has a dead host, 6 are the literal string `"(pending)"`, and the freshness clock's default "today" is frozen at the same date every source was verified. §8 |

---

## 1. What the drill subsystem actually is

Three gates, one summary, one sample stream. No part of it touches the
milling load model.

| Artefact | File | Built from |
|---|---|---|
| `DrillSample` (per peck per hole) | `drill_metrics.rs:27-52` | re-expansion of `DrillOp.cycle` (`peck_descents`, `:308`) |
| `DrillToolpathSummary` | `drill_metrics.rs:78-111` | the sample stream + `DrillOp` |
| `DrillGatesVerdict{chip_welding, peck_adequacy, plunge_feed}` | `tool_load/drill_gates.rs:68-85` | the summary + `DrillOp` + `Material` |
| Narration | `narrate.rs:1522-1596` | the summary + `classify_plunge_feed` |
| Diagnostics | `diagnostics/adapters/from_tool_load.rs:411-508` | the verdict |
| GUI remedy strings | `viz/src/ui/sim_diagnostics.rs:1089-1099` | `CriterionKind` |
| Export gating | `drill_gates.rs:244-281` → `verdict.rs:256-264` | `Critical` only |

The milling gates (`chipload`, `power`, `deflection`) slot
`Unmodeled(NotApplicableForOp)` for drill ops, so **nothing on the drill path
consumes `f_lut`, `mean_chip_factor`, an `ae` band, or any LUT row.** B-lit's
condition (1) is confirmed by construction, not by inspection alone — see §4.

---

## 2. Gate-vs-evidence matrix

`t` = the material threshold returned by the named accessor. `d` = tool
diameter (see §2.4 for what "diameter" means here). All ratios dimensionless
unless stated.

### 2.1 Chip welding

| Field | Value |
|---|---|
| Evidence | `DrillToolpathSummary::chip_welding_dtd` (`drill_metrics.rs:99`) |
| Formula | `effective_chip_welding_dtd(total_dtd, per_peck_max_dtd, cycle)` (`:283-289`) — `Simple`/`Dwell` → total; `ChipBreak` → total × 0.5; `Peck` → deepest single peck |
| Statistic | max over holes (total) or max over pecks (peck cycles). Not a mean, not a percentile |
| Threshold source | `Material::drill_chip_welding_threshold_dtd` (`material.rs:1041-1070`) |
| Band structure | half-open: Low `[0, 0.75t)`, Elevated `[0.75t, t)`, High `[t, ∞)` (`classify_chip_welding`, `:295-304`) |
| Displayed threshold | **`t`, for all three bands** (`drill_gates.rs:106`) |
| Severity map | Low→`Within`; Elevated→`Exceeds(Elevated)`→`LoadState::Within`; High→`Exceeds(Critical)`→`LoadState::Exceeds` (blocks export) |
| Diagnostic id | `drill.chip_welding`, label `"Chip welding (D/d)"`, unit `"ratio"` |
| Confidence | `Approximate` (honest) |

**Material bands** (`janka_to_drill_chip_welding_dtd`, `material.rs:565-573`):

| Class | `t` | Source |
|---|---|---|
| Softwood, Janka ≤ 700 | 8.0 | **none** — see §6.2 |
| Medium hardwood, 700 < Janka ≤ 1500 | 6.0 | **none** |
| Dense hardwood / out-of-band / NaN, Janka > 1500 | 5.0 | **none** |
| Plywood, SheetGood | 5.0 | **none** |
| Plastic | 4.0 | **none** |
| Aluminum | 3.0 | "industry rule of thumb", inline, uncited |
| Foam | 12.0 | uncited |
| Fiberglass | 3.0 | uncited placeholder, **disclosed** in `CREDITS.md:311` |
| Custom | `(8.0 / max(feed_scale,0.5)).clamp(2,12)` | derived, uncited |

### 2.2 Peck adequacy

| Field | Value |
|---|---|
| Evidence | **re-derived in the gate**, not read from the summary: `evaluate_peck_adequacy` (`drill_gates.rs:135-167`) recomputes `peak_peck_dtd` from `DrillOp.cycle` + `summary.deepest_hole_mm` |
| Duplicate | `build_drill_toolpath_summary` computes the same quantity as `per_peck_max_dtd` (`drill_metrics.rs:236-239`) and stores only the derived boolean `peck_pattern_adequate` — **the number itself is never exposed** |
| Formula | `Simple`/`Dwell` → `deepest_hole/d`; `Peck`/`ChipBreak` → `min(peck, deepest_hole)/d` |
| Threshold source | `Material::drill_per_peck_max_dtd` (`material.rs:1077-1099`) |
| Band structure | one-sided: `Within` at `≤ t`, `Exceeds(Critical)` above. **No advisory band** — unlike chip welding, there is no 0.75 warning tier |
| Severity map | `Critical` only → blocks export |
| Diagnostic id | `drill.peck_adequacy`, label `"Peck adequacy (peck/D)"`, unit `"ratio"` |

**Material bands** (`janka_to_drill_per_peck_max_dtd`, `material.rs:596-604`):

| Class | `t` | Suggest default = `t × 0.5` |
|---|---|---|
| Softwood ≤ 700 | 6.0 | 3.0 × D |
| Medium hardwood ≤ 1500 | 5.0 | 2.5 × D |
| Dense / unknown > 1500 | 4.0 | 2.0 × D |
| Plywood / SheetGood | 1.5 | 0.75 × D |
| Plastic | 1.0 | 0.5 × D |
| Aluminum | 1.0 | 0.5 × D |
| Foam | 4.0 | 2.0 × D |
| Fiberglass | 1.0 | disclosed placeholder |
| Custom | 1.5 | — |

The wood row's stated basis (`material.rs:583-584`) is
*"literature 3–8×D — Onsrud Drill Chart, FPL Wood Handbook §3.7, Vectric drill
defaults"*. §6.2 shows **all three legs fail**, and §6.3 shows the "3–8×D"
figure is a *total-hole* regime number applied as a *per-peck* ceiling.

Note the internal inconsistency this creates: the same physical quantity
(depth-to-diameter) carries **8.0 as a total-hole ceiling and 6.0 as a
per-peck ceiling** for softwood. A single 7×D hole drilled `Simple` is
`Elevated` on chip welding and `Exceeds(Critical)` on peck adequacy — two
gates, one number, opposite severities.

### 2.3 Plunge feed

| Field | Value |
|---|---|
| Evidence | `feed_rate_mm_min / d`, units mm/min per mm Ø |
| Formula | `classify_plunge_feed` (`drill_gates.rs:180-220`) — the single implementation, shared with `narrate` (F1) |
| Threshold source | `Material::drill_plunge_feed_envelope_per_mm` (`material.rs:1136-1153`) |
| Band structure | two-sided. `< lo` → `Exceeds(Elevated)`; `> hi` → `Exceeds(Critical)`; else `Within` with `threshold` = **nearer bound** and the full envelope alongside |
| Severity map | low side never blocks export; high side does |
| Diagnostic id | `drill.plunge_feed`, unit `"mm/min per mm Ø"` |

**Envelopes**: SolidWood (50, 400) · Plywood/SheetGood (40, 350) · Plastic
(60, 500) · Aluminum (40, 250) · Foam (100, 1000) · Fiberglass (40, 200) ·
Custom (40, 500).

Stated provenance (`material.rs:1123-1131`): *"Onsrud wood-drilling bulletin
and Vectric drill defaults give 0.08–0.18 mm/tooth × 2 flutes at the 8–14 kRPM
small-drill band ≈ 210–430 mm/min per mm Ø … the 50 floor is the rubbing onset
… (FPL Wood Handbook Ch.19)"*. §6.2: no such Onsrud bulletin was found; the
"0.08–0.18 mm/tooth" figure is the *literature-matrix cell's own band*, which
§6.4 shows was fitted to this code; and FPL Ch.19 is *Specialty Treatments*,
verified §6.2.

**Unit consistency (research question 3): PASS.** Four independent sites agree
on mm/min per mm Ø with no conversion between them — `material.rs:1136`,
`feeds/mod.rs:1377-1378` (Step 9c clamp, multiplies by `d`),
`drill_gates.rs:186`, `narrate.rs:1570`, and the matrix invariant
`drill_final_feed_in_plunge_envelope` (`expr = "feed_rate / D"`, floor 50,
ceiling 400). This is the one axis of the drill subsystem that is *not*
exposed to T3.1.

### 2.4 The shared "diameter" — and where it is wrong

Every ratio above divides by `DrillOp::tool_diameter_mm`, set at
`compute/execute.rs:396` as `tool_def.radius() * 2.0`, with
`tool_profile: ToolProfile::Flat` **hardcoded regardless of the assigned tool**
(`:420`, `:449`).

`radius()` is the **ENVELOPE** radius by contract (`tool/mod.rs:174-186`): "for
a tapered ball it is the SHAFT radius … it overstates the cutter's reach at
finishing depth by up to 14×". No precondition in `catalog.rs` restricts a
Drill or AlignmentPinDrill op to a flat/twist tool.

Consequence: assign a tapered-ball or V-bit to a drill op and *all three*
gates divide by a diameter up to 14× larger than the cutting diameter — D/d,
peck/D and feed/D all read up to 14× **low**, and every gate reports `Within`
on a grossly overloaded cutter. This is the same defect class as the
tech-debt-1 radius programme (`project_radius_tech_debt_programme`); it is
recorded here as a **cross-reference**, not claimed as new.

---

## 3. Cycle time and the model/emitter split — the one MODEL_FIX with hard arithmetic

`DrillToolpathSummary::{peck_count, feed_time_s}` are computed by re-expanding
the *config* (`peck_descents`, rooted at `hole.top_z`). The emitted G-code is
produced independently by `drill::drill_peck_full_retract` (`drill.rs:86-120`),
whose peck grid is rooted at **`retract_z`, the R-plane** — which is the
Fanuc G83 convention and is correct.

`retract_z` is not the config value. `execute.rs:651` passes it through
`effective_safe_z(cfg.retract_z, stock_top)` =
`max(raw, stock_top + SAFE_Z_CLEARANCE_MM)` with `SAFE_Z_CLEARANCE_MM = 5.0`
(`compute/config.rs:999-1008`). The `DrillConfig` default is 2.0, so the
R-plane is **always `stock_top + 5.0`** for any default project.

Worked example, shipped defaults (`DrillConfig::default()` — depth 10, Peck(3),
feed 300, Ø6, `stock_top = 0`, R = +5):

| | emitted G-code | `DrillToolpathSummary` |
|---|---|---|
| feed-down moves | 5 (ending +2, −1, −4, −7, −10) | `peck_count = 4` |
| first move | **100 % air** (+5 → +2) | not represented |
| total feed distance | 15.0 mm | 10.0 mm |
| feed time @300 mm/min | 3.00 s | `feed_time_s = 2.00 s` |
| retract round trips | 5 | not reported |
| deepest single *cutting* descent | 3.0 mm | 3.0 mm ✔ |

So: **`peck_count` understates by 25 %, `feed_time_s` by 33 %**, both printed
verbatim by `narrate.rs:1544-1562`. The gate-relevant number (deepest single
peck) survives, which is why this has never tripped a gate.

It gets worse at small peck depths. At the GUI minimum `peck_depth = 0.5`
(`viz/ui/properties/operations/drill.rs:146`), the first **ten** feed-downs are
pure air — ten retract round trips per hole that remove nothing, invisible to
every drill metric, and invisible to the air-cut channel too because
`OperationType::air_cut_high_threshold_pct` is `None` for Drill.

The structural fix already exists and is unused: `RegionSpanRole::DrillHole` /
`DrillPeck` spans are emitted for every drill toolpath
(`compute/spans.rs:218-250`, derived geometrically from XY grouping +
`MoveType::Linear`, **not from labels**) and the span walker counts all 5
feed moves. **Two shipped peck counts disagree by construction**, and the
correct one is the one nothing reads.

---

## 4. Does any drill threshold read a LUT band? — No, and it cannot

Census of the bundled LUT (`crates/rs_cam_core/data/vendor_lut/`), by
`operation_family`:

```
total rows 256
   36  adaptive      50  contour       9  face
   31  parallel      97  pocket        5  scallop      28  trace
    0  drill
```

`LutOperationFamily::Drill` exists in the schema (`vendor_lut.rs:240-247`) with
a docstring describing a 2026-06-02 fix, but **no row was ever authored for
it**, and `vendor_lookup::passes_must_match` (`:459-462`) returns `false`
immediately on family mismatch. A Drill query is a guaranteed miss.

Three consequences worth recording:

1. **B-lit's condition (1) is confirmed structurally.** Softening the LUT
   diameter/hardness exponents (T3.4) cannot move any drill number, because no
   drill number is ever read from a LUT row. The only inheritance is
   `feed_scale_factor` through the *formula* path, which B-lit §4.2 bounds at
   ≤ 0.71 %.
2. **`vendor_lut.rs:804-830`'s reachability test is one-directional.** It
   asserts every *row's* family is queryable; nothing asserts every *queryable*
   family has rows. Drill is the silent inverse case.
3. **Two shipped sentries guard an unreachable mechanism.**
   `_litmatrix_drill_rpm_ceiling.rs` names "the vendor-RPM override (Step 2)"
   as one of two paths that lifted drill RPM past 14 k. That path cannot fire
   for a drill query. Only the `MaxSpeed` speedup arm is live. The sentries
   still pass and still guard something real; their *stated* mechanism is half
   fictional.

---

## 5. `DRILL_CHIPLOAD_MULTIPLIER = 2.5` — verdict

`feeds/mod.rs:826-840`. Applies only on the formula path, only for
`OperationFamily::Drill`, on top of `k0·D^p·(1/feed_scale)^q`.

The comment states three things. All three are checkable, and none holds.

**Claim A — "drill chipload bands are ~2.5× higher than milling chipload at
similar D".** No source is given. §6 finds no retrievable source that states
any such ratio. Against the only primary wood-drill chart that exists
(Onsrud 72-000, retrieved §6.1) the required factor is **4.76–5.41, geometric
mean 4.97**:

| D (mm) | Onsrud 72-000 Wood (mm/tooth) | code, ×2.5, softwood | code ÷ Onsrud lo | multiplier to centre |
|---|---|---|---|---|
| 3 | 0.229 – 0.279 | 0.117 | 0.51 | 5.41 |
| 5 | 0.279 – 0.330 | 0.160 | 0.57 | 4.76 |
| 6 | 0.330 – 0.381 | 0.179 | 0.54 | 4.97 |
| 8 | 0.381 – 0.432 | 0.213 | 0.56 | 4.76 |

So the factor is **directionally right and roughly half the size** the one
available chart implies. It is not an over-correction.

**Confound, stated:** the Onsrud wood row is footnoted *"Gang drills run at
4,500 RPM and 150 IPM"* — a rigid multi-spindle production borer with short
brad-point bits, not a hobby router with a collet and 5× the stickout. This
document therefore **does not recommend raising the multiplier to 5**. It
recommends that the number stop claiming a provenance it does not have.

**Claim B — "a softwood drill at 12k RPM × milling-formula chipload lands at
~0.03 mm/rev".** Arithmetically false. With the shipped
`ChipLoadFormula::default()` (`machine.rs:35-42`: k0 0.024, p 0.61, q 1.26) and
`GenericSoftwood` (Janka 600 → `feed_scale = 1.0`):

| D | milling `f_z` | per-rev @ 2 flutes |
|---|---|---|
| 3 mm | 0.0469 | **0.094** mm/rev |
| 6 mm | 0.0716 | **0.143** mm/rev |
| 12 mm | 0.1093 | **0.219** mm/rev |

The un-multiplied formula reads 3.1×–7.3× the quoted 0.03, and already sits
*inside or above* the "0.05–0.15 mm/rev drilling band" the comment says it
falls below.

**Claim C — "audit finding: implied chipload 0.026 on Wanaka Pin Drill /
Holes".** That observation is real, but it is an *implied* chipload —
`feed/(rpm × flutes)` of the **stored** op, i.e. the value after the milling
plunge baseline clobbered the drill-tuned feed. `feeds/mod.rs:1368-1375`
records that exact mechanism and states that **Step 9c fixed it** by aliasing
`plunge_rate` to the drill feed. So the 2.5× and the Step 9c clamp are two
overlapping corrections for one defect, and only Step 9c addresses the
mechanism.

**Verdict: THRESHOLD_RECALIBRATION deferred; documentation defect now.** The
value should not move at Checkpoint D — moving it without settling the
gang-drill confound would swap one unsourced number for another. What must
change is the comment: it asserts a numeric premise that its own shipped
constants contradict.

**T3.1 dependency: REAL.** This is the one drill number that rides the formula
chipload. It must not be re-derived until the gate reports `fz_eff` and the
optimizer targets are re-derived (B-lit §6.1).

---

## 6. Sources, freshness, and lineage

Method: `/refresh-lit-matrix` policy — retrieve the primary document, quote
verbatim, record the access date, and record failures in a NOT VERIFIED
ledger rather than inferring.

### 6.1 The real Onsrud drill chart — retrieved

`onsrud_drill`'s `citation_url` (`sources.toml:155`,
`www.onsrud.com/files/pdf/drill_chart.pdf`) is **HTTP 404**, tested
2026-08-04. The live equivalent was located by fetching Onsrud's own
cutting-data index (`onsrud.com/Forms/Cutting-Data-Recommendations.asp`, HTTP
200) and reading its PDF link list. The drill chart is:

> **<https://www.onsrud.com/images/Drill.pdf>** — retrieved 2026-08-04,
> HTTP 200, 756 398 bytes, page headed *"Drill Cutting Data Recommendations"*,
> footer `124  www.onsrud.com` (page 124 of the PCT-19 catalogue).

Verbatim content of the one wood row, column positions recovered from the
PDF's own text bounding boxes:

> Series **72-000\***, material **Wood**, no SFM column value.
> Recommended Chip Load per Tooth by Cutting Diameter (in):
> **3 mm → .009-.011 · 5 mm → .011-.013 · 6 mm → .013-.015 · 8 mm → .015-.017**
> Footnote: **"\* Gang drills run at 4,500 RPM and 150 IPM"**
> Formulas block: `RPM = (3.82 x SFM) / tool dia.` · `Feedrate (IPM) = RPM x IPR`
> Definitions: `IPM = Inches Per Minute` · `IPR = Inches Per Revolution`

The chart is **numerically self-verifying and confirms B-lit's T4.1 answer
independently**: `150 IPM / (4 500 RPM × 2 flutes) = 0.01667 in/tooth`, which
lands exactly inside the printed `.015-.017` top-diameter band. The column is
an advance per tooth, at 2 flutes.

Regressing the four printed midpoints gives a **diameter exponent of 0.485**
for drilling — inside B-lit's cross-vendor range [0.23, 1.25], near its
row-weighted centre 0.52, and below the shipped `p = 0.61`. Recorded as a data
point only; a four-point single-series regression is not grounds to move `p`.

**What the chart does not contain:** any peck guidance, any depth-of-hole or
depth-to-diameter guidance, any RPM band other than the fixed 4 500 gang-drill
figure, and any per-species wood split. A case-insensitive search for
`peck` / `depth of hole` / `hole depth` returns **0** on the retrieved text,
and also 0 on the retrieved `Hard Wood.pdf`, `Soft Wood.pdf`,
`Hard Plywood.pdf` and `Hard Plastic.pdf`. The `sources.toml` note
*"Drill IPR / peck guidance"* (`:161`) is false for the document.

Onsrud's drill series index (`onsrud.com/Series/HSSandSolidCarbideDrills.asp`,
retrieved) lists chip-breaker drill series and publishes **no** feed/speed,
peck or hole-depth data.

### 6.2 The FPL Wood Handbook citations are fabricated

Three code sites and four matrix cells cite `fpl_wood_handbook` for drilling.

- `sources.toml:205` cites **FPL-GTR-282 (2021)** at
  `www.fpl.fs.fed.us/documnts/fplgtr/fpl_gtr282.pdf` — the host **does not
  resolve** (`getaddrinfo ENOTFOUND`, both `curl` and `WebFetch`; the USDA FS
  retired the `fs.fed.us` domain in favour of `research.fs.usda.gov`).
  Publication confirmed live at
  <https://research.fs.usda.gov/treesearch/62200> (retrieved 2026-08-04):
  *"Wood handbook—wood as an engineering material"*, FPL-GTR-282, 2021, 543 pp.
- GTR-282's chapter list (same page + search corroboration) runs
  1 Renewable Resource … 3 **Structure and Function of Wood** … 18 Fire Safety
  … 20 Heat Sterilization. There is **no machining or drilling chapter**, and
  **no §3.7 on drilling**.
- The 2010 edition **GTR-190** was retrieved in full
  (`ww2.arb.ca.gov/sites/default/files/barcu/regact/2014/capandtradeprf14/usfswoodhandbook2010.pdf`,
  HTTP 200, 15.8 MB, 2026-08-04) and searched. Chapter 19 is
  **"Specialty Treatments"**. Every one of the 26 occurrences of the string
  "peck" is *pecky cypress* or *bird peck* — wood defects. "Drilling" appears
  only in fastener pilot-hole and preservative-treatment contexts.

**Therefore, verified absent:** the FPL Wood Handbook contains no drilling
feeds, no peck guidance, and no depth-to-diameter guidance, in either edition.
Every one of the following citations is unsupported by the document it names:

| Site | Cited as | Reality |
|---|---|---|
| `material.rs:583` | "FPL Wood Handbook §3.7" for the 3–8×D per-peck band | §3 is *Structure and Function of Wood*; no §3.7 on drilling |
| `material.rs:1128` | "FPL Wood Handbook Ch.19" for the 50 mm/min·mm rubbing floor | Ch.19 is *Specialty Treatments* |
| `feeds/mod.rs:690-693` | "FPL Wood Handbook Ch.19 (drilling)" for the RPM tiers | same |
| `_litmatrix_drill_rpm_diameter_tier.rs:6` | "FPL Wood Handbook Ch.19" for the 3–8k big-drill band | same |
| `cells.toml` × 4 slots | `sources = [… "fpl_wood_handbook"]` on drill feed/peck bands | source has no drilling content |

Also recorded (inherited from B-lit, re-confirmed): `sources.toml` cites
GTR-**282** while the scaling functions the feeds stack actually uses are
GTR-**190** Table 5–11a, already cited in the LUT manifest as `fpl_ch5_2010`.
Two source records, one publication, different reports.

### 6.3 Where "3–8×D" actually comes from — and the misreading

`CREDITS.md:436` lists, under *acceptance benchmark seed sources* (explicitly
"not … new bundled runtime LUT data"):

> CNC Cookbook deep-hole drilling reference: <https://www.cnccookbook.com/deep-hole-drilling/>

Retrieved 2026-08-04. Verbatim:

> *"Most tooling manufacturers regard any depth that is more than 3 or 4 times
> the diameter of the twist drill as a deep hole."*
> *"Twist Drills can go down 5 diameters deep without issue. From 5 to 7
> diameters, you should use Peck Drilling."*
> *"You can go a lot deeper with a parabolic flute drill — 20 x Diameter vs
> only 7 x Diameter."*

This is the closest match in the repo to `material.rs:583`'s "literature
3–8×D", and it is the **total-hole depth regime at which pecking becomes
necessary** — the chip-welding axis. It says nothing about how deep an
individual peck may be. The page is for metal twist drills and carries no
material banding.

The independent machining-literature convention for per-peck depth, gathered
this wave (secondary, recorded as such): full peck (G83) first peck 0.5–1.0×D
then 0.5×D steps; chip-break (G73) 0.5–1.0×D. That is **6× to 12× shallower**
than the repo's softwood per-peck ceiling of 6.0×D.

**Finding:** a total-hole regime figure was transcribed into a per-peck
ceiling, attributed to two sources that do not contain it, and then also used
as the total-hole ceiling in the sibling gate. One number, two incompatible
roles, no source for either role.

### 6.4 The literature-matrix drill cells were fitted to the code, then cited

`cells.toml` drill cells (`flat_3mm_drill_oak` `:396`, `flat_6mm_drill_oak`
`:1603`, `flat_3mm_drill_softwood` `:2136`, `flat_12mm_drill_oak_big`) band
`feed_per_tooth` at **0.08 – 0.18 mm/tooth**, `sources = ["onsrud_drill",
"vectric_drill_default", "fpl_wood_handbook"]`.

Against the code, red oak (Janka 1290), formula × 2.5:

| D | code output | matrix band |
|---|---|---|
| 3 mm | **0.0797** | 0.08 – 0.18 |
| 6 mm | **0.1217** | 0.08 – 0.18 |
| 12 mm | **0.1857** | 0.08 – 0.18 |

The band brackets the implementation's own output to three decimal places at
both ends, and the Ø3 cell sits 0.0003 mm/tooth **below** its own floor.

Against the source it cites, Onsrud 72-000 Wood spans **0.229 – 0.432
mm/tooth**. `0.18 / 0.229 = 0.79` — the band and its cited source **do not
overlap at any diameter**.

Same pattern on RPM: the invariant `drill_rpm_floor` has `floor = 8000`,
`severity_on_fail = "critical"`, `sources = ["onsrud_drill", "amana_spektra"]`.
Onsrud's only published wood-drill RPM is **4 500**. The invariant would fail
its own cited source's recommendation, at critical.

And `peck_depth_under_hardwood_max` (ceiling 15.0 mm) and `peck_over_d_hardwood`
(ceiling 5.0) both cite `onsrud_drill` + `fpl_wood_handbook` for peck data that
neither document contains (§6.1, §6.2).

### 6.5 `onsrud_drill` / `vectric_drill_default` lineage — FEEDS_CENSUS §7 gap 5

Confirmed still open. Searching `CREDITS.md` for drill/peck returns three hits:
the Fiberglass placeholder disclosure (`:311-312`) and the CNC Cookbook
acceptance-benchmark line (`:436`). **Neither `onsrud_drill` nor
`vectric_drill_default` has any `CREDITS.md` entry**, and
`vectric_drill_default`'s URL is the bare docs root `https://docs.vectric.com/`
(HTTP 200 but non-specific — it resolves to a documentation portal, not to any
drill default table).

**Correct replacement lineage, proposed for the `/refresh-lit-matrix` run:**

| Row | Today | Proposed |
|---|---|---|
| `onsrud_drill` | `files/pdf/drill_chart.pdf` (404), notes claim "IPR / peck guidance" | `https://www.onsrud.com/images/Drill.pdf`, retrieved 2026-08-04. Notes rewritten to what it contains: *per-tooth chip load by cutting diameter for series 72-000 Wood (0.009–0.017 in/tooth, Ø3–8 mm), 70-500 Plastic and four composite series; gang drills fixed at 4,500 RPM / 150 IPM; RPM and feedrate formulas.* **No peck, hole-depth or depth-to-diameter guidance.** |
| `onsrud_hwood`, `onsrud_doc_rule` | `series_70_85.pdf` (404) | `https://www.onsrud.com/images/Hard%20Wood.pdf` (200) |
| `onsrud_swood` | `series_60_softwood.pdf` (404) | `https://www.onsrud.com/images/Soft%20Wood.pdf` (200) |
| `onsrud_plywood` | `series_70_plywood.pdf` (404) | `https://www.onsrud.com/images/Hard%20Plywood.pdf` / `Soft%20Plywood.pdf` (200) |
| `onsrud_plastic` | `plastic_chart.pdf` (404) | `https://www.onsrud.com/images/Hard%20Plastic.pdf` / `Soft%20Plastic.pdf` (200) |
| `fpl_wood_handbook` | GTR-282 at dead `fs.fed.us` host | `https://research.fs.usda.gov/treesearch/62200`; and **drop it from every drill cell's `sources`** — it contains no drilling content |
| `vectric_drill_default` | `https://docs.vectric.com/` | no specific published drill-default table was located; either cite the exact page or restate the row as repo-authored community practice |
| `dapra_rctf`, `kennametal_metals`, `kennametal_chipload` | `kennametal.com/…/calculators.html` (404) | not investigated this wave — outside drill scope, flagged for the same run |
| `toolgrit` | `toolgrit.com/feeds-speeds-calculator` (404) | same |

There is **no primary source for the per-peck ceiling**, in any form, that this
wave could locate. The honest replacement is the CNC Cookbook total-hole
regime for the *chip-welding* axis (already in `CREDITS.md`, community tier,
metal twist drills), plus an explicit statement that the per-peck band is
**repo-authored** — the same treatment B-lit gave the repo-authored `ae_rule`
strings.

---

## 7. Diagnostic wording audit — four shipped contradictions

### C-1 · A `Caution` diagnostic that says "exceeds" about a value below its own threshold — **the money finding**

`diagnostics/adapters/from_tool_load.rs:493`:

```rust
message: format!("{label} exceeds: {observed:.2} vs {threshold:.2}")
```

Chip welding maps `ChipWeldingRisk::Elevated` — band `[0.75t, t)`, i.e.
*observed strictly less than threshold* — to `DrillGateOutcome::Exceeds`
(`drill_gates.rs:118-124`), whose `threshold` field is documented as
**"The violated bound."** Nothing was violated.

Shipped output, softwood (`t = 8.0`), a Ø3 tool with a 22 mm peck (the fixture
of the existing test `oversize_peck_fails_peck_adequacy`, `drill_gates.rs:372`,
`chip_welding_dtd = 22/3 = 7.33`):

> **`Chip welding (D/d) exceeds: 7.33 vs 8.00`** — `Severity::Caution`

This is not a corner. The Elevated band is 25 % of the threshold wide by
construction, and F1's own note (`drill_gates.rs:376-383`) records the live
WANAKA pin drill landing inside it at 4.5 vs 6.

The same string is emitted on the plunge-feed **low** side, where the value is
below the floor: `Plunge feed envelope exceeds: 16.67 vs 50.00`. There the
number is the genuinely violated bound, but the verb points the wrong way.

Root cause: one `Exceeds` variant carries three distinct relations —
*below floor*, *inside the warning band below the ceiling*, and *above the
ceiling*.

### C-2 · A `Within` verdict displaying a threshold it was not compared against

`drill_gates.rs:106` sets `threshold = t` for **all three** chip-welding bands,
but `Low` is decided at `0.75t`. A Ø4 × 23.6 mm softwood hole reads:

> `Chip welding (D/d) within (5.90)` · evidence `GeometryCompare { lhs 5.90, rhs "threshold" 8.00 }`

Implied headroom 26 %. Real headroom to the next band: **1.7 %**. This is
exactly the acceptance gate's "a Within verdict cannot display an observed
value outside a hard bound without an explicit explanation", inverted — the
displayed *bound* is not the one that decided the verdict.

The plunge-feed gate already solved this problem correctly, in the same file
(`drill_gates.rs:28-40`, `205-218`): it carries `envelope_lo` / `envelope_hi`
alongside `threshold` and documents precisely why ("pre-F1 this overload made
an at-the-floor reading look like a healthy headroom number"). Chip welding
passes `None` for both.

### C-3 · F1's defect, alive on the sibling gate

F1 (2026-06-10) fixed a chip-welding verdict whose remedy said *"switch to a
peck cycle"* on an op that was already pecking. The **peck-adequacy** gate has
the mirror image and it was not fixed.

`evaluate_peck_adequacy` (`drill_gates.rs:141-150`) trips on `Simple`/`Dwell`
cycles using the **whole-hole** ratio. Ø4 × 30 mm softwood `Simple` →
`7.5 > 6.0` → `Exceeds(Critical)`. The GUI then prints
(`viz/src/ui/sim_diagnostics.rs:1094`):

> *"single peck too deep for the material — reduce peck depth"*

on a cycle that has no peck depth. The correct remedy is "switch to a peck
cycle". Symmetrically, `DrillChipWelding`'s remedy (`:1090`) is unconditional:

> *"hole depth-to-diameter exceeds the material chip-welding threshold —
> switch to a peck cycle or reduce depth"*

Post-F1, reaching `High` on a `Peck` cycle requires `peck/D ≥ t`, which is
achievable inside GUI bounds (Ø1 tool, peck 10 mm → 10.0 > 8.0) — and the
advice tells a pecking op to start pecking. Both remedies are reachable
simultaneously on one hole, giving the operator two contradictory instructions.

### C-4 · Narrate prints "chips cleared" beside "chip evacuation inadequate"

`chip_evacuation_score` returns a hard **1.0 for every `Peck` cycle regardless
of peck depth** (`drill_metrics.rs:145`). `avg_chip_evacuation_score` is
therefore 1.00 for any pecking op. On the `Peck(22.0)`, Ø3, 25 mm fixture,
`narrate.rs:1544-1562` emits, two lines apart:

> `ℹ drill cycle — … peck pattern INADEQUATE — reduce peck depth.`
> `ℹ drill cycle time: … mean chip-evacuation score 1.00 (0=trapped, 1=cleared).`

A score whose own legend says `1 = cleared`, printed next to a verdict saying
evacuation is inadequate. The score's `Peck` arm ignores the depth that the
verdict is about.

### Wording gaps (no contradiction, but the operator cannot check the work)

- Narrate never prints the chip-welding **threshold**, only the risk word. The
  reader gets `depth-to-diameter 4.5× total / 4.5× evacuation-credited
  (chip-welding risk elevated)` and no way to know whether the bar was 5, 6 or 8.
- Narrate never prints the peck-adequacy **ratio or threshold** — only the
  boolean.
- `DrillToolpathSummary` exposes `peck_pattern_adequate` but not the
  `per_peck_max_dtd` it was derived from (`drill_metrics.rs:229-247`), so no
  consumer can display the number behind the boolean.
- `ChipBreak` displays `chip_welding_dtd = total × 0.5`. A 15×D `ChipBreak`
  hole in softwood displays `7.50 vs 8.00 → Within`. The field doc is honest
  ("the value the risk was actually classified from") and `max_depth_to_diameter`
  is carried alongside, so this is a *labelling* risk rather than a
  contradiction — but the 0.5 factor is itself uncited.
- Drill `Exceeds` diagnostics carry `SampleRange { sample_start: 0,
  sample_end: 0, locality: default }` (`from_tool_load.rs:494-501`). Drill
  samples exist and are keyed `(hole_id, peck_index)`; the evidence cannot lead
  the operator to the offending hole.

---

## 8. The `citation_url` validation gap — census and proposal

### 8.1 Census (all 32 rows, `sources.toml`, tested 2026-08-04)

| Status | Count | Rows |
|---|---|---|
| **HTTP 404** | **10** | `onsrud_hwood`, `onsrud_doc_rule`, `onsrud_swood`, `onsrud_plywood`, `onsrud_plastic`, `onsrud_drill` (all gold); `toolgrit` (community); `dapra_rctf`, `kennametal_metals`, `kennametal_chipload` (all gold, one shared dead URL) |
| **Host does not resolve** | 1 | `fpl_wood_handbook` (gold) — `fs.fed.us` retired |
| **Literal `"(pending)"`** | 6 | `whiteside_wood`, `whiteside_chipload` (gold); `fanuc_kc_table`, `ineos_hdpe_machining`, `plastic_machining_guide`, `aluminum_machining_guide` (engineering) |
| 403 / bot-blocked (undetermined) | 3 | `amana_spektra`, `amana_vbit` (shared URL), `woodweb` |
| Unreachable from this sandbox (undetermined) | 2 | `shapeoko_wiki`, `cutter_shop` |
| **HTTP 200 / 202** | 10 | `vectric_default`, `vectric_drill_default`, `gwizard_hwood`, `gwizard_swood`, `gwizard_plywood`, `shopbot`, `shaw_metal_cutting`, `shaw_chipload`, `onsrud_soft_plastic_cutting_data`, `amana_plastic_oflute` |

**B-lit reported 4 dead gold URLs. The true count is at least 11 dead rows,
9 of them gold tier**, plus the 6 `"(pending)"` placeholders — **17 of 32
source records (53 %) do not resolve to a document.**

Every one of them is reported `fresh`.

### 8.2 Why nothing catches it — two independent holes

**Hole 1 — `citation_url` is never read.** `freshness.rs:136-176` reads exactly
one field, `last_verified`. `audit_citations` (`:284-339`) validates that a
cell's `sources = [...]` keys *exist in `sources.toml`* — a referential-integrity
check on keys, not on URLs. The identifier `citation_url` appears nowhere in
the freshness or runner code.

**Hole 2 — the freshness clock is frozen at the seed date.**
`DEFAULT_TODAY = "2026-06-03"` (`freshness.rs:30`) is the same date that **30 of
32** rows carry as `last_verified`; the other two are 2026-05-29, which the
day-of-month rule also floors to age 0. Absent `LIT_MATRIX_TODAY`, every row
computes `age_months = 0` and classifies `Fresh` — permanently, by
construction. A default CI run cannot produce a `warn` or `stale` row no matter
how much time passes. (W3's census ran it at `LIT_MATRIX_TODAY=2026-08-04` and
got `{"fresh":32,"warn":0,"stale":0}` — 2 months, still fresh; the freeze has
not yet *bitten*, but it is a permanently-fresh clock waiting to.)

And even when a row does go stale, it only fails under
`LIT_MATRIX_DECAY_FAIL=1` (`runner.rs:72-78`), which nothing sets.

### 8.3 Proposal (report-only; no network in CI)

Four parts, ordered by cost. None adds a dependency; none adds a network call
to CI.

**P1 — offline URL-shape validation, as a hard test failure.** Cheap, pure
string work, catches the 6 `"(pending)"` rows immediately and any future
placeholder. Rules: field present; non-empty; not `"(pending)"` or any
`(...)`-shaped sentinel; parses as `http(s)://host/…`; host contains a dot.
Fail the matrix run on violation, same tier as the existing citation audit
(which already fails unconditionally, `runner.rs:66-71`). This is the
"test-only URL-shape check" the brief permits.

**P2 — an explicit `url_verified` date, separate from `last_verified`.** The
present field conflates two questions: *"is the number still what the source
says?"* and *"does the link still work?"*. Splitting them lets a URL rot be
detected without re-reading the whole chart, and makes the `/refresh-lit-matrix`
checklist enumerable. Freshness reports both; both age.

**P3 — liveness as an opt-in, off by default.** `LIT_MATRIX_CHECK_URLS=1`
performs HEAD requests and writes a report; CI never sets it; the
`/refresh-lit-matrix` skill sets it as step 1 of its walk. This keeps the
network out of the default gate while making the check a single command
instead of a manual 32-row sweep. If even opt-in network in the test binary is
unwanted, the equivalent is a committed shell one-liner in the skill — the
census in §8.1 was produced by exactly that.

**P4 — un-freeze the clock.** `DEFAULT_TODAY` should not be a hardcoded past
date. Either derive "today" from the build (breaking CI determinism, probably
unwanted) or — better — keep it deterministic but **assert that it is not
older than the newest `last_verified` in the file**, so the freeze is
self-detecting the moment someone re-verifies a source.

`/refresh-lit-matrix` policy alignment: the skill's premise is "re-verify or
replace stale rows … roughly once a year". That premise depends on the
freshness report being able to say `stale`. Today it cannot. P4 is the
prerequisite for the skill working at all.

---

## 9. Synthetic drill matrix

The brief asks for fixtures that distinguish shallow/valid, per-peck excess,
depth excess, high plunge, low plunge, and a material-boundary transition
**without relying on a label**, with populations selected by `RegionSpanRole`.

**Executability: NOT RUN — slot unavailable.** The Cargo slot was held by W4
(`adversarial_2d_campaign_r2 --ignored`) for the whole wave, with a foreign
`sysml-runtime` job alongside; `free -g` read 9–10 GB available at every check,
below the 10 GB bar. Commands recorded in §11.

All three gates are **closed-form arithmetic over config + material constants**
— no simulation, no dexel, no geometry — so the expected verdicts below are
derived exactly, not estimated. Each is stated with the arithmetic that
produces it, so an execution pass is a confirmation, not a discovery.

Material for D1–D5: `Material::default()` = `SolidWood { GenericSoftwood }`,
Janka 600 → chip-welding `t = 8.0`, per-peck `t = 6.0`, plunge envelope
`(50, 400)`.

| # | Fixture | Ø | depth | cycle | feed | chip welding | peck adequacy | plunge feed |
|---|---|---|---|---|---|---|---|---|
| **D1** | shallow / valid | 6 | 18 | `Peck(2)` | 300 | `Within` 0.33 vs 8.0 | `Within` 0.33 vs 6.0 | `Within` 50.0, envelope 50–400 |
| **D2** | per-peck excess | 3 | 25 | `Peck(22)` | 300 | `Exceeds(Elevated)` **7.33 vs 8.00** ← C-1 | `Exceeds(Critical)` 7.33 vs 6.0 | `Within` 100.0 |
| **D3** | total-depth excess, no peck | 4 | 40 | `Simple` | 300 | `Exceeds(Critical)` 10.0 vs 8.0 | `Exceeds(Critical)` 10.0 vs 6.0 ← C-3 wrong remedy | `Within` 75.0 |
| **D4** | plunge too fast | 3 | 6 | `Peck(1)` | 1500 | `Within` 0.33 | `Within` 0.33 | `Exceeds(Critical)` 500 > 400 |
| **D5** | plunge too slow | 6 | 12 | `Peck(2)` | 100 | `Within` 0.33 | `Within` 0.33 | `Exceeds(Elevated)` 16.67 < 50 ← C-1 wording |
| **D6** | **material boundary** | 6 | 40 | `Simple` | 300 | see below | see below | `Within` 50.0 |

D1–D5 exist today as unit tests in `drill_gates.rs:330-461`; the matrix's
contribution is D6 plus the *evidence* assertions (D2 and D5 currently assert
only the severity, never the displayed numbers, which is why C-1 has shipped).

### D6 — the label-proof boundary fixture

`WoodSpecies::RadiataPine`, Janka **710 lbf** (`material.rs:31`, corrected
2026-05-30 from 500 per Wood Database). It is the only shipped species in the
narrow band just above the softwood cut-off.

| Selector | Value |
|---|---|
| Botanical class | **softwood** (a pine) |
| `Material::category()` (GUI picker, splits at 900) | **`MaterialCategory::Softwood`** |
| `janka_to_drill_chip_welding_dtd` (splits at 700) | **medium hardwood → 6.0** |
| `janka_to_drill_per_peck_max_dtd` (splits at 700) | **medium hardwood → 5.0** |

So a Ø6 × 40 mm `Simple` hole reads D/d = 6.67:

- as `GenericSoftwood` (600) → `t = 8.0` → `6.67 < 8` and `≥ 0.75·8 = 6` → **Elevated**
- as `RadiataPine` (710) → `t = 6.0` → `6.67 ≥ 6` → **High / Critical, blocks export**

**A 110 lbf Janka step flips the gate from a warning to an export blocker, on
two species the GUI files in the same drawer, both labelled softwood.** The
fixture asserts the verdict from the *Janka value*, and asserts that the GUI
category label and the gate band disagree — so it cannot pass by reading a
label. It also pins the three-way disagreement of the repo's own hardness
partitions: **700/1500** for the drill bands, **900** for the picker, and
**600** for `feed_scale_factor`'s baseline.

### Role-typed populations

`RegionSpanRole::{DrillHole, DrillPeck}` already exist and are emitted for
every drill toolpath (`compute/spans.rs:218-250`), derived geometrically. The
matrix should select its per-hole and per-peck populations through
`Span::has_region_role`, exactly as `compute/execute.rs:4429-4434` already
does — **never** through `Span::label`, whose contract
(`toolpath_spans.rs:288-291`) forbids downstream dependence.

Doing so makes §3's defect a *test failure* rather than an observation: the
span walker counts 5 feed-downs on the default fixture where
`DrillToolpathSummary::peck_count` reports 4. That single assertion is the
whole of R-2 below.

### Out-of-band robustness note (not a gate finding)

`peck_descents` (`drill_metrics.rs:308-326`) does `peck.max(f64::MIN_POSITIVE)`
and then `remaining -= step`, which never converges for a zero or
sub-epsilon peck — unbounded `Vec` growth. `drill::drill_peck_full_retract`
(`drill.rs:96-119`) has the same shape with no guard at all, and also loops
forever for a **negative** peck. The GUI clamps `peck_depth` to `0.5..=50.0`,
but `ParamDef::required("peck_depth", "f64")` (`catalog.rs:1278`, `:1501`)
carries no range, so MCP `set_toolpath_param` and hand-edited project TOML can
reach it. Same class as W0's PR-1 adaptive3d livelock. **Owner: W0/W8, not a
Checkpoint D item.** Recorded so it is not lost.

Related asymmetry: `apply_drill_defaults` (`suggest.rs:892-915`) applies
`clamp_peck_to_depth` to `Drill` but **not** to `AlignmentPinDrill`, so a
softwood Ø6 pin drill gets an 18 mm Suggest peck against a typically 13 mm
hole — a single-shot cycle wearing a `Peck` label, which the gate then passes
at 2.17 vs 6.0.

---

## 10. Recommendations, per gate

| # | Item | Recommendation | Rationale | T3.1 dep |
|---|---|---|---|---|
| **R-1** | Chip-welding gate — Elevated rendered as `"exceeds: 7.33 vs 8.00"` | **REPORT_FIX** | The verdict contradicts the number printed beside it, at `Caution`, in shipped GUI text. Fix is a third outcome (`Advisory` / `Approaching`) or a message that names the band boundary; the physical bands do not move | No |
| **R-2** | `peck_count` / `feed_time_s` model the config, not the emitted R-plane-rooted cycle | **MODEL_FIX** | 25 % / 33 % understatement on shipped defaults (§3), and the correct data (`DrillPeck` spans) is already emitted and unread. Build the summary from spans; add the role-typed sentry first, red | No |
| **R-3** | Chip-welding `Within`/`Exceeds` display `t` when the band boundary is `0.75t` | **REPORT_FIX** | Same class as the plunge gate's own documented F1 fix; carry `band_lo`/`band_hi` the way `envelope_lo`/`envelope_hi` already are | No |
| **R-4** | Peck-adequacy remedy on `Simple`/`Dwell`; chip-welding remedy on `Peck` | **REPORT_FIX** | F1's exact defect, unfixed on the sibling gate and un-conditioned on the original. Make both remedies switch on `DrillCycle` | No |
| **R-5** | `chip_evacuation_score` returns 1.0 for `Peck` at any depth | **MODEL_FIX** | Prints "cleared" beside "INADEQUATE" in one narrate block (C-4). The `Peck` arm should fall off with *per-peck* D/d, mirroring what the F1 chip-welding credit already does | No |
| **R-6** | Peck-adequacy ratio computed twice, exposed zero times | **REPORT_FIX** | Plan §"Preferred fix shape" item 2. Put `per_peck_max_dtd` on `DrillToolpathSummary`; have the gate read it; print it in narrate | No |
| **R-7** | Drill `Exceeds` evidence is `SampleRange 0..0` | **REPORT_FIX** | Drill samples are keyed `(hole_id, peck_index)`; the evidence should name the hole | No |
| **R-8** | Chip-welding thresholds 8 / 6 / 5, plywood 5, plastic 4 | **NO_CHANGE** (this wave) + documentation defect | The values are in the right order of magnitude against the one retrievable regime statement (CNC Cookbook: 5×D fine, 5–7×D peck) but their cited sources contain no such data (§6.2). Nothing has been shown to be *wrong*; what is wrong is the citation. Do not move numbers on the strength of a metal-twist-drill blog post | No |
| **R-9** | Per-peck ceilings 6 / 5 / 4, plywood 1.5, plastic 1.0 | **THRESHOLD_RECALIBRATION — flagged, not proposed** | The stated basis is a *total-hole* regime figure used as a *per-peck* ceiling (§6.3), the general machining convention is 0.5–1.0×D (6–12× shallower), and the wood rows have no source at all. This is the drill subsystem's weakest number. It needs primary evidence before it moves in either direction — recalibrating it downward on metal-drilling practice would be the same error in reverse | No |
| **R-10** | Plunge envelopes (50, 400) etc. | **NO_CHANGE** + documentation defect | The one unit-consistent axis (§2.3), agreed across four sites. Its stated provenance is circular: it cites a "0.08–0.18 mm/tooth" figure that is the matrix cell's own band, which §6.4 shows was fitted to this code | No |
| **R-11** | `DRILL_CHIPLOAD_MULTIPLIER = 2.5` | **Documentation defect now; THRESHOLD_RECALIBRATION deferred** | Comment's arithmetic premise is false by 3–7× (§5 Claim B); the defect it names was separately fixed by Step 9c; against the one real chart it is ~2× *under*-sized, on a gang-drill confound that forbids acting on that number | **YES** — the only real one |
| **R-12** | `tool_diameter_mm = radius()×2` with `ToolProfile::Flat` hardcoded, no tool precondition on drill ops | **MODEL_FIX — cross-reference** | ENVELOPE radius on a tapered ball overstates by up to 14×, making all three gates read `Within` on an overloaded cutter. Belongs to the radius programme, not to a drill threshold change | No |
| **R-13** | `sources.toml` — 10 hard 404s (9 gold), 1 dead host, 6 `"(pending)"` | **REPORT_FIX, immediate** | §6.5 gives the retrieved live replacements for every Onsrud row and for FPL. Re-open condition is *now*, not annual | No |
| **R-14** | Drill cells cite `fpl_wood_handbook` and `onsrud_drill` for data neither contains; bands fitted to the code | **REPORT_FIX** | §6.2, §6.4. Drop `fpl_wood_handbook` from every drill cell; rewrite `onsrud_drill`'s note to what the chart holds; mark the `feed_per_tooth` band and the peck invariants **repo-authored**, the same treatment B-lit gave the `ae_rule` strings | No |
| **R-15** | `drill_rpm_floor = 8000` at `severity_on_fail = "critical"`, cited to `onsrud_drill` | **REPORT_FIX** | The cited chart's only wood-drill RPM is 4 500. The invariant would fail its own source at critical. Either the citation goes or the floor's real (hobby-router-spindle) basis is stated | No |
| **R-16** | Freshness engine: `citation_url` unvalidated; clock frozen at seed date; decay never fails | **REPORT_FIX** | §8.3 P1–P4. P1 alone catches 6 rows today at zero cost and no network | No |
| **R-17** | `LutOperationFamily::Drill` has 0 of 256 rows; reachability test is one-directional | **REPORT_FIX** | Add the inverse assertion so a query family with no rows is a stated fact rather than a silent permanent miss, and correct `_litmatrix_drill_rpm_ceiling.rs`'s docstring, which names an unreachable mechanism | No |

**Counts: NO_CHANGE 2 · REPORT_FIX 10 · MODEL_FIX 4 · THRESHOLD_RECALIBRATION
1 flagged + 1 deferred.**

Every REPORT_FIX is inside the plan's §"Preferred fix shape" item 1 ("fix
evidence/wire/wording contradictions separately from physical thresholds") and
inside Checkpoint B's Q2 report-only tier: none moves an operator-actionable
number. **No drill policy, default, severity or threshold is proposed to move
before Checkpoint D.**

---

## 11. NOT RUN — slot unavailable

The Cargo slot was held for the entire wave. `free -g` / bracketed `pgrep`
checked at start, mid-wave and before writing; never cleared (W4
`adversarial_2d_campaign_r2 --ignored`, plus a foreign `sysml-runtime` job;
available memory 9–10 GB, below the 10 GB bar). W6 is third in priority.

| # | Command | What it would confirm |
|---|---|---|
| 1 | `cargo test -p rs_cam_core --lib drill_gates` | the 8 in-file gate tests still pass at HEAD; baseline 2233/0 |
| 2 | `cargo test -p rs_cam_core --lib drill_metrics` | the 11 summary/classification tests |
| 3 | `cargo test -p rs_cam_core --test drill_metrics_pr2` | end-to-end session → summary → gate |
| 4 | `cargo test -p rs_cam_core --test drill_material_plumbing_f016` | per-material threshold plumbing |
| 5 | `cargo test -p rs_cam_core --test drill_op_step3` | drill op emission |
| 6 | `cargo test -p rs_cam_core --test _litmatrix_drill_rpm_ceiling` | §4's claim that only the MaxSpeed arm is live |
| 7 | `cargo test -p rs_cam_core --test _litmatrix_drill_rpm_diameter_tier` | tier behaviour |
| 8 | `LIT_MATRIX_TODAY=2026-08-04 cargo test -p rs_cam_core --test literature_matrix -- --nocapture` | the drill cell rows and the freshness/citation report at the real date (W3 recorded 19 passed, `{"fresh":32,"warn":0,"stale":0}` on 2026-08-04 — §8.2's freeze claim is arithmetic over the file, not an observation of a failure) |

Everything asserted in this document is either (a) arithmetic over
file-resident constants, shown inline with its inputs, (b) a census of a
committed data file, whose method is stated in §4 / §8, or (c) a retrieved
primary document with its URL, HTTP status, byte count and access date.
Nothing rests on a test outcome.

Known-red state unchanged and untested: `wanaka_suggest_baseline` remains
environmental per plan §2 rule 9.

---

## 12. NOT VERIFIED ledger

Per `/refresh-lit-matrix` policy — what was attempted, what failed, what the
claim rests on instead.

| id | Claim | Attempted | Status |
|---|---|---|---|
| **W6-N1** | `amana_spektra` / `amana_vbit` / `woodweb` `citation_url` liveness | `curl -L` with a browser UA → **HTTP 403** (bot block); the same publisher-level 403 B-lit hit on `amanatool.com/pub/media/…` | **UNDETERMINED.** Not counted as dead in §8.1. A `pub/media` Amana PDF *did* return 200, so the host is up |
| **W6-N2** | `shapeoko_wiki`, `cutter_shop` liveness | `curl` → connection refused / DNS failure from this sandbox | **UNDETERMINED.** Not counted as dead. `fpl.fs.fed.us` *is* counted, because its DNS failure was independently reproduced through `WebFetch` (a different network path) and corroborated by the publisher's documented move to `research.fs.usda.gov` |
| **W6-N3** | That the Onsrud 72-000 wood row is 2-flute | The chart prints no flute count | **INFERRED, self-consistently.** `150 IPM / (4 500 × 2) = 0.01667 in/tooth` lands inside the printed `.015-.017` band; at 3 flutes it would read 0.0111, outside every column on the row |
| **W6-N4** | That the Ø3/5/6/8 columns on the drill chart are millimetres while 1/8, 1/4, 3/8… are inches | The header row mixes both, with "(in)" qualifying the *values* not the columns | **VERIFIED by column geometry.** Positions recovered from the PDF's word bounding boxes; the composite/plastic rows populate only the inch columns and the wood row only the metric ones, consistent with metric-shank wood gang drills |
| **W6-N5** | That no primary source states a per-peck depth-to-diameter limit for **wood** | Searched Onsrud (all 5 retrieved charts + drill series index), FPL GTR-190 full text, FPL GTR-282 chapter list, `sources.toml`, `CREDITS.md`, `research/`, general web | **NO PRIMARY SOURCE LOCATED.** The per-peck bands 6/5/4/1.5/1.0 are repo-authored. Recorded as a negative result, deliberately |
| **W6-N6** | `vectric_drill_default`'s underlying data | `https://docs.vectric.com/` returns 200 but is a portal root; no drill-defaults table located | **NOT VERIFIED.** Row is `authority_tier = "community"`; its own note already says "representative of community CAM practice" |
| **W6-N7** | The per-peck 0.5–1.0×D machining convention quoted in §6.3 | Search-engine summaries of tooling-vendor and forum sources; no vendor PDF retrieved | **SECONDARY ONLY.** Used to size the *gap*, never as a recalibration target — which is precisely why R-9 is flagged rather than proposed |
| **W6-N8** | Whether the 403/undetermined rows in §8.1 would raise the dead count above 11 | Not resolvable without a browser session | **BOUNDED BELOW.** §8.1's "at least 11" is a floor, not an estimate |

---

## 13. Checkpoint D decision list — drill side

Nothing here is urgent in the sense of a wrong number reaching a spindle. The
gates' *physics* is defensible; their *paperwork* and their *reporting* are not.

1. **Approve the report-only tier (R-1, R-3, R-4, R-6, R-7, R-13…R-17) to
   proceed without further ruling.** Ten items, none moves an operator-actionable
   number, all inside Checkpoint B Q2's already-granted report-only tier. R-1
   is the one an operator can see today.
2. **Approve R-2 and R-5 as MODEL_FIX with red-first sentries.** Both change
   reported numbers (peck count, cycle time, evacuation score) without changing
   any threshold. R-2's sentry is the role-typed span-vs-summary comparison in
   §9; it should be committed red before the fix, per
   `feedback_commit_instruments_before_gates`.
3. **Rule on R-9 (per-peck ceilings).** This is the only threshold this wave
   would move, and this wave explicitly declines to propose a direction. The
   question for the operator is whether to commission primary evidence (a bench
   trial, or a retrievable wood-drilling source this wave did not find) or to
   accept the numbers as declared repo-authored heuristics with the citation
   corrected. **Recommendation: the latter, this cycle** — correct the citation,
   keep the number, and let a future bench trial move it.
4. **Confirm R-11's deferral.** `DRILL_CHIPLOAD_MULTIPLIER` is the only drill
   number with a real T3.1 dependency. Fix its comment now; do not touch its
   value until T3.1 lands and the optimizer targets are re-derived.
5. **Note for the R-12 / radius-programme owner**: drill ops accept any tool and
   divide by the envelope radius. Not a drill threshold change; it belongs in
   that programme's queue.

The single sentence a reader should take away: **the drill gates measure the
right quantities in the right units, and then tell the operator something that
is not true about the answer** — most visibly in
`Chip welding (D/d) exceeds: 7.33 vs 8.00`.
