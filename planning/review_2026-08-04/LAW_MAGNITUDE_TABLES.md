# Law magnitude tables — what adopting `D^0.61` and `Janka^-0.5` would cost

Date: 2026-08-06
Wave: **conversion** (second tech-debt programme, branch `experiment/adaptive-spiral`)
Basis: Checkpoint B **Q4** — *"one literature task … implementation returns as a
separate approval with magnitudes"*
(`planning/review_2026-08-04/ORCHESTRATION_LOG.md:39`).
Recommendation being sized: `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` §4.

> ## STATUS: MEASURE-ONLY. NOTHING HERE IS ADOPTED.
>
> `feeds::vendor_lookup::CHIPLOAD_DIAMETER_EXPONENT` and
> `CHIPLOAD_HARDNESS_EXPONENT` both ship at **1.0** and this wave did not
> move them. Every "proposed" number below is computed *in a test* by
> applying `0.61` / `0.5` to the same raw ratios production uses. The
> harness is committed: `crates/rs_cam_core/tests/law_magnitude_measurement.rs`
> (`#[ignore]`d; run with `-- --ignored --nocapture`).
>
> **This document asks for a decision. It does not record one.**

---

## 0. What the recommendation is, in one paragraph

B-lit §4 regressed the *shipped vendor charts themselves* — no vendor publishes
an exponent — and found the LUT path's `^1.0` diameter law and `^1.0` hardness
law are both outliers. It recommends the **LUT path adopt the formula path's
existing exponents**, so only one implementation moves: diameter `D^0.61`
(already `machine::ChipLoadFormula::default().p`), hardness `Janka^-0.5` (within
0.71 % of the formula path's true composite `^0.504` across the whole calibrated
Janka band). Blast radius: `vendor_lookup::{diameter_ratio_raw → applied scale,
hardness_ratio_raw → applied scale}` and nothing else.

Both changes **raise** the band when a row is transferred *down* in diameter or
*up* in hardness, and lower it the other way. On small tools that is a
loosening, not a tightening — which is what the vendors' own charts say, and
which B-lit argues is the correct direction because under-predicting chipload
commands too-slow feed, and in wood too-slow feed is the rubbing-and-burning
failure mode rather than a conservative margin.

---

## 1. The one thing that had to ship first, and did

The recommendation carries a trap B-lit named as a **mandatory rider**:
`is_extrapolated` was computed on the *applied* scale, so softening an exponent
shrinks the product toward 1.0 and silently un-flags rows that are exactly as
far from the query as they were. Un-flagging downgrades
`Confidence::Approximate` → `Validated` and, via
`ChipBoundsSource::low_side_is_advisory`, converts burn **advisories into hard
`Exceeds(Low)` trips**.

**Measured across the shipped LUT** (248 chipload-bearing rows × a grid of 8
query diameters × 6 query Jankas = 11 712 pairs):

| | count | share |
|---|---:|---:|
| pairs flagged today | — | — |
| pairs that would **silently lose the flag** under `^0.61` / `^0.5` | **932** | **8.0 %** |

Named examples (raw ratios unchanged; only the applied product moves):

| row | query | raw d | raw h | applied today | applied proposed |
|---|---|---:|---:|---:|---:|
| `amana-tapered-mdf-parallel-3175-2f` | Ø1.0, Janka 500 | 0.3150 | 2.200 | 0.693 (flagged) | 0.733 (**not** flagged) |
| `amana-flat-mdf-pocket-0794-2f-spektra` | Ø1.0, Janka 500 | 1.2598 | 1.400 | 1.764 (flagged) | 1.362 (**not** flagged) |
| `amana-flat-softwood-pocket-1587-2f-spektra` | Ø1.0, Janka 500 | 0.6299 | 1.000 | 0.630 (flagged) | 0.754 (**not** flagged) |
| `idcwoodcraft-cm-18-compression` | Ø1.0, Janka 500 | 0.3150 | 2.200 | 0.693 (flagged) | 0.733 (**not** flagged) |

**This wave moved the flag onto the raw ratios** (`vendor_lookup::is_extrapolated_for_ratios`,
red-first sentry `tests/chipload_extrapolation_flag_rider.rs`), so the 8.0 % is
now a *number that was avoided*, not a cost the approval has to absorb. With the
rider in place, **adopting the exponents changes no extrapolation flag at all.**

---

## 2. Diameter law — `D^1.0 → D^0.61`

### 2.1 Whole-LUT sweep

Band multiplier = `(query/row)^0.61 ÷ (query/row)^1.0`, over the 244
chipload-bearing rows carrying a diameter anchor. Ratios clamped to
`[0.1, 10]` exactly as production clamps them.

| query Ø (mm) | min × | p25 | **median** | p75 | max × |
|---:|---:|---:|---:|---:|---:|
| 1.000 | 0.914 | 1.838 | **2.056** | 2.409 | 2.455 |
| 1.5875 | 0.763 | 1.535 | **1.717** | 2.011 | 2.455 |
| 2.000 | 0.697 | 1.403 | **1.569** | 1.838 | 2.455 |
| 3.175 | 0.582 | 1.171 | **1.310** | 1.535 | 2.250 |
| 6.000 | 0.454 | 0.914 | **1.022** | 1.198 | 1.756 |
| 6.350 | 0.444 | 0.894 | **1.000** | 1.171 | 1.717 |
| 12.000 | 0.407 | 0.697 | **0.780** | 0.914 | 1.340 |
| 12.700 | 0.407 | 0.682 | **0.763** | 0.894 | 1.310 |

The pivot is Ø6.35 — the LUT's most-populated diameter — where the median
multiplier is exactly 1.000. Below it the change **raises** bands; above it, it
**lowers** them.

### 2.2 Named worst rows

| direction | row | row Ø | query Ø | multiplier |
|---|---|---:|---:|---:|
| most **raised** | `whiteside-vbit-mdf-trace-12000-2f` | 12.0 | 1.0 | **×2.455** |
| most raised (large-Ø row) | `whiteside-facing-softwood-face-25000-2f` | 25.0 | 1.5875 | ×2.455 (clamp-bound) |
| most raised at Ø3.175 | `idcwoodcraft-su-10-surfacing` | 25.4 | 3.175 | ×2.250 |
| most **lowered** | `amana-flat-mdf-pocket-0794-2f-spektra` | 0.79375 | 12.0 | **×0.407** |
| most lowered (ball) | `amana-ball-mdf-parallel-0794-3f-zrn` | 0.794 | 12.7 | ×0.407 |

Both extremes sit **at the `SCALE_CLAMP_LO`/`HI` boundary**, i.e. on transfers
that are already absurd (a Ø25 surfacing row answering a Ø3 query; a Ø0.79 micro
row answering a Ø12.7 query). Every one of them is `is_extrapolated == true`
today and stays so under the rider. That matters for the approval: the largest
multipliers land on rows the verdict already labels `Approximate` and whose low
side is already advisory-only.

---

## 3. Hardness law — `Janka^1.0 → Janka^0.5`

Band multiplier = `(row_J/query_J)^0.5 ÷ (row_J/query_J)^1.0`, over all 248
chipload-bearing rows, using the same `family_default_janka` fallback production
uses (softwood 500, hardwood 1290, MDF 700, …).

| query Janka | material | min × | **median** | max × | most-lowered row | most-raised row |
|---:|---|---:|---:|---:|---|---|
| 500 | SPF / pine | 0.587 | **0.845** | 1.000 | `amana-ball-hardwood-parallel-3175-2f` ×0.587 | (none raised) |
| 600 | soft maple-ish | 0.643 | **0.926** | 1.095 | same ×0.643 | `onsrud-softwood-60-800-3_8-roughing` ×1.095 |
| 700 | MDF | 0.695 | **1.000** | 1.183 | same ×0.695 | same ×1.183 |
| 1290 | red oak | 0.943 | **1.000** | 1.606 | same ×0.943 | same ×1.606 |
| 1450 | hard maple | 1.000 | **1.060** | 1.703 | — | same ×1.703 |
| 3510 | **Ipe** | 1.000 | **1.650** | **2.650** | — | `onsrud-softwood-60-800-3_8-roughing` ×2.650 |

Read the two ends:

- **Into softwood** (the common case — 122 Amana rows, most hardwood-anchored):
  the change is **conservative**, median ×0.845. Bands drop.
- **Into Ipe** (Janka 3510, 2.7× beyond any chart's "hard wood"): the change is
  **permissive**, median ×1.650 and up to ×2.650 on softwood-anchored rows.

That second row is the one to look hardest at, and it is where this document
declines to make a recommendation of its own — see §5.

---

## 4. Combined, on realistic queries

Over the same 11 712 (query, row) pairs, `applied_new / applied_old`:

| direction | row | query | raw d | raw h | multiplier |
|---|---|---|---:|---:|---:|
| most lowered | `whiteside-sc64-conical-ball-nose-spiral-fusion360` | Ø12.7, Janka 500 | 8.807 | 2.580 | **×0.267** |
| | `amana-ball-mdf-parallel-0794-3f-zrn` | Ø12.0, Janka 500 | 10.000 (clamped) | 2.200 | ×0.275 |
| most raised | `onsrud-softwood-60-000lh-1_2-roughing` | Ø1.0, Janka 3510 | 0.100 (clamped) | 0.1425 | **×6.504** |
| | `onsrud-softwood-60-200-3_4-finish` | Ø1.5875, Janka 3510 | 0.100 (clamped) | 0.1425 | ×6.504 |

**Every one of the six extremes is clamp-bound on the diameter axis**, i.e. it
describes a transfer the lookup should arguably refuse rather than scale. The
×6.504 row is an Onsrud ½"–¾" **softwood** row answering a Ø1 mm **Ipe** query;
that is not a law problem, it is a row-selection problem, and it is the same
class as census T4.4 (pass role: filter or score?) and T4.6
(`family_default_janka`'s provenance). Recorded here because an approval that
looks only at the headline multiplier will attribute it to the exponent.

---

## 5. Per-cell effect on the committed literature-matrix sentries

Three `_litmatrix_*` cells consume a scaled chipload band. (The others —
`_litmatrix_milling_rpm_diameter_tier`, `_litmatrix_drill_rpm_ceiling`,
`_litmatrix_drill_rpm_diameter_tier`, `_litmatrix_scallop_refuses_flat` — are
RPM-ceiling or refusal cells and consume no chipload band at all.)

Measured 2026-08-06 by
`cargo test -p rs_cam_core --test law_magnitude_measurement -- --ignored --nocapture`
at commit-in-progress on `experiment/adaptive-spiral`:

| cell | winning row | raw d | raw h | band today (mm/tooth) | band proposed | band × | flag today / proposed |
|---|---|---:|---:|---|---|---:|---|
| `_litmatrix_ipe_janka_scaling` / **oak** (Ø3 flat 2F pocket rough) | `amana-zrn-flat-mdf-pocket-3175-2f` (Ø3.175) | 0.9449 | 0.8088 | 0.05824–0.09706 | 0.06620–0.11033 | **×1.137** | false / false |
| `_litmatrix_ipe_janka_scaling` / **ipe** (Ø3 flat 2F pocket rough) | `freud-solid-carbide-eighth-hardwood` (Ø3.175) | 0.9449 | 0.3675 | 0.01764–0.04410 | 0.02975–0.07437 | **×1.686** | true / true |
| `_litmatrix_rubbing_floor_clamp` / **ipe** (Ø6 flat 2F pocket rough) | `amana-flat-hardwood-pocket-6000-2f` (Ø6.000) | 1.0000 | 0.4131 | 0.01322–0.02272 | 0.02057–0.03535 | **×1.556** | true / true |
| `_litmatrix_rubbing_floor_clamp` / **oak control** (Ø6 flat 2F pocket rough) | `amana-flat-hardwood-pocket-6000-2f` (Ø6.000) | 1.0000 | 1.0662 | 0.03412–0.05864 | 0.03304–0.05679 | ×0.968 | false / false |
| `_litmatrix_rpm_only_lut_chipload` (Ø12 flat 4F adaptive rough, oak) | `onsrud-hardwood-60-000hh-1_2-roughing` (Ø12.700) | 0.9449 | 0.9485 | 0.38700–0.43253 | 0.40625–0.45404 | ×1.050 | false / false |
| B3 live cell (Ø1 tapered ball 2F scallop finish, hard maple 1450) | `amana-tapered-hardwood-scallop-3175-2f` (Ø3.175) | 0.3150 | 1.0000 | 0.00378–0.00756 | 0.00593–0.01186 | **×1.569** | true / true |
| chipload-gate in-module cell (Ø6 flat 2F pocket rough, hard maple 1450) | `amana-flat-hardwood-pocket-6000-2f` (Ø6.000) | 1.0000 | 1.0000 | 0.03200–0.05500 | 0.03200–0.05500 | ×1.000 | false / false |

Two notes on reading this table:

- **The flag column never moves.** That is the rider working (§1): every
  entry reads the same before and after, and the `raw-rule` value the harness
  prints alongside agrees with the shipped flag on every cell. Without the
  rider, three of these seven would have been at risk.
- **The B3 band here (0.00378–0.00756) is the *unde­rated* lookup band.** The
  gate's own band on that fixture is 0.003605–0.007211 because it queries at
  `lookup_diameter_at(peak axial DOC)` = 0.954 mm rather than the nominal
  1.0 mm, and then applies `doc_derating_scale`. The multiplier is the same.

### 5.1 What each sentry would be exposed to, structurally

- **`_litmatrix_ipe_janka_scaling`** asserts only `ipe_fpt < oak_fpt` — a
  **directional** assert. Under `^0.5` the Ipe hardness scale is
  `0.3675^0.5 = 0.606 < 1.0`, so the direction holds. **No re-pin needed.** Its
  docstring's *"derate Ipe … by roughly 1290 / 3510 ≈ 0.37×"* narrative would go
  stale and must be corrected in the same commit that moves the exponent.
  (B-lit §4.2 corrects the census on this: the census listed a re-pin as
  required; it is not.) **This wave already fixed the docstring's other defect
  — see §8.**
- **`_litmatrix_rubbing_floor_clamp`** asserts the Ipe Ø6 pocket chipload is
  clamped **up** to the 0.025 mm/tooth rubbing floor and that a
  `ChiploadClampedToFloor` warning fires. **Measured**: the Ipe band moves
  0.01322–0.02272 → 0.02057–0.03535 (×1.556), so the *midpoint* Suggest reads
  moves 0.01797 → 0.02796 — **above the 0.025 floor**. The clamp would then
  **stop firing**, `ipe_pocket_emits_chipload_clamped_warning` would go RED, and
  `ipe_pocket_chipload_never_drops_below_rubbing_floor` would pass for a
  different reason.

  **This is the sharpest single consequence in this document and it is a
  re-pin, not a regression**: the sentry exists to prove the clamp fires when
  the derate takes a chipload below the chip-formation floor, and under the
  proposed hardness law this cell no longer goes below the floor. The cell must
  either be moved to a harder/smaller combination that still derates under
  0.025, or restated as "the clamp fires when and only when the derated
  chipload lands below the floor". **Deciding that is part of approving the
  law**, and it must not be discovered after the exponent lands.

  It is also the reason §6 item 3 asks for the rubbing floor to be re-derived
  first: the sentry pins the *interaction* of a band and a floor, and the
  proposal moves the band while Checkpoint B Q3 left the floor held.
- **`_litmatrix_rpm_only_lut_chipload`** exercises the *formula fallback* for a
  row publishing no chipload band. The exponents never apply. **Unaffected.**

### 5.2 The interaction the approval must not miss

`RUBBING_FLOOR_MM_TOOTH = 0.025` sits **above** the derated band maximum on the
B3 row — 2.73× today, and B-lit computes **1.71×** after both laws land. The
laws *reduce* that contradiction and do not resolve it. Two shipped policies
still disagree about the same number, and Checkpoint B Q3 **held** the rubbing
floor pending the unit ruling. That ruling has now landed (this wave), so
**the floor's re-derivation is unblocked** and should be sequenced *before* the
exponents, not after: moving the band and moving the floor in either order
produces different sentry outcomes, and the floor is the side with the weaker
provenance.

---

## 6. The decision this document asks for

1. **Adopt `D^0.61` in `vendor_lookup`?** Cost: bands ×2.06 median at Ø1,
   ×0.78 median at Ø12; extremes clamp-bound and already flagged
   `Approximate`. Benefit: one law instead of two, and the LUT path stops
   under-predicting permissible chipload on small tools relative to every
   vendor family in the LUT except Freud.
2. **Adopt `Janka^-0.5` in `vendor_lookup`?** Cost: ×0.845 median into
   softwood (conservative), ×1.65 median into Ipe (permissive), ×2.65 worst.
   Benefit: same unification; the formula path moves ≤ 0.71 % so only one
   implementation changes.
3. **Sequence against the rubbing floor.** The recommendation here is to
   re-derive `RUBBING_FLOOR_MM_TOOTH` **first**, because the exponents change
   what "the band" is and the floor is currently asserted against it by
   `_litmatrix_rubbing_floor_clamp` with a margin that the exponents halve.
4. **Nothing in §4's clamp-bound extremes is a law problem.** If the ×6.5
   Onsrud-softwood-row-answering-an-Ipe-query transfer is unacceptable, the
   lever is row selection (census T4.4 / T4.6), not the exponent.

---

## 7. Raw harness output

Whole-LUT diameter sweep, verbatim (244 chipload-bearing rows with a diameter
anchor; the harness's second test):

```text
| query Ø | min × | p25 | median | p75 | max × | most-lowered row | most-raised row |
|---:|---:|---:|---:|---:|---:|---|---|
| 1.0000  | 0.914 | 1.838 | 2.056 | 2.409 | 2.455 | `amana-flat-softwood-pocket-0794-2f-spektra` ×0.914 | `idcwoodcraft-su-10-surfacing` ×2.455 |
| 1.5875  | 0.763 | 1.535 | 1.717 | 2.011 | 2.455 | `amana-flat-softwood-pocket-0794-2f-spektra` ×0.763 | `idcwoodcraft-su-10-surfacing` ×2.455 |
| 2.0000  | 0.697 | 1.403 | 1.569 | 1.838 | 2.455 | `amana-flat-softwood-pocket-0794-2f-spektra` ×0.697 | `idcwoodcraft-su-10-surfacing` ×2.455 |
| 3.1750  | 0.582 | 1.171 | 1.310 | 1.535 | 2.250 | `amana-flat-softwood-pocket-0794-2f-spektra` ×0.582 | `idcwoodcraft-su-10-surfacing` ×2.250 |
| 6.0000  | 0.454 | 0.914 | 1.022 | 1.198 | 1.756 | `amana-flat-softwood-pocket-0794-2f-spektra` ×0.454 | `idcwoodcraft-su-10-surfacing` ×1.756 |
| 6.3500  | 0.444 | 0.894 | 1.000 | 1.171 | 1.717 | `amana-flat-softwood-pocket-0794-2f-spektra` ×0.444 | `idcwoodcraft-su-10-surfacing` ×1.717 |
| 12.0000 | 0.407 | 0.697 | 0.780 | 0.914 | 1.340 | `amana-ball-softwood-parallel-1000-2f-zrn` ×0.407  | `idcwoodcraft-su-10-surfacing` ×1.340 |
| 12.7000 | 0.407 | 0.682 | 0.763 | 0.894 | 1.310 | `amana-ball-softwood-parallel-1000-2f-zrn` ×0.407  | `idcwoodcraft-su-10-surfacing` ×1.310 |
```

The hardness sweep and the combined/extrapolation figures in §1, §3 and §4 come
from the same arithmetic run over the 20 shipped observation JSON files directly
(252 rows, 248 chipload-bearing); the harness reproduces the diameter axis
through the shipped `apply_chipload_law` so the two agree by construction.

---

## 8. What this wave DID change, so §5 is read correctly

This wave shipped the **unit conversion** (B-lit §3: the post-sim chipload gate
now observes `effective_feed / (rpm · flutes)`) and the **mandatory rider**
(§1). It did **not** touch either exponent.

One thing in §5's list moved anyway: `_litmatrix_ipe_janka_scaling`'s docstring
was corrected, per B-lit §4.2's instruction and the census's P11 "stale
rationale outliving the code" class. That correction is about the *census's*
claim that the cell needed a re-pin; it does not pre-empt the exponent decision.
