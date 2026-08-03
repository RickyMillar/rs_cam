# Superseded conclusions — the H4 re-measurement ledger

**C-sequence wave 15 (H4), 2026-08-04.** Basis:
`TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` item H4 and Addendum B §B.3
(Checkpoint E). Branch `experiment/adaptive-spiral`.

This document is the deliverable H4 defines and L1.6 asks for: **one row per
claim that was produced through a broken instrument**, saying which defect
invalidates it, whether it has been re-measured, and what the re-measurement
found.

## How to read a row

A row here means the claim is **void, not falsified**. Those are different
things and the difference is the whole point of this wave:

* *Falsified* — we measured, and the claim is wrong.
* **Void** — the comparison **could not have come out any other way**. The
  instrument decided the answer before the experiment ran. A void claim may
  well be true; nothing here says otherwise. It has simply never been
  tested, and it must not be cited as though it had been.

`NOT RE-RUN` is therefore an honest terminal state for a row, provided the
reason is stated. It is not a to-do marker.

---

## 1. The four instrument defects

| # | Defect | What it did to a measurement | Fixed |
|---|---|---|---|
| **D1** | Classification grid **6x too coarse** — the cell size was derived from the tool's SHANK radius, not its CUSP radius | Band decomposition ran at 0.75 mm instead of 0.125 mm on the shipped Ø1-tip taper. Ribbons of steep terrain merged or vanished before any strategy saw them | `32c5e48` (2026-07-29) |
| **D2** | Finish-planner **dials 6x/36x too large** — `min_region_area_mm2` 144 vs 4 mm², `close_radius_mm` 1.5 vs 0.25, `pencil_claim_floor` 0.75 vs 0.125, all from `radius()` instead of `cusp_radius()` | An area floor 36x too high deleted small regions wholesale; a close radius 6x too big merged distinct regions into one blob. Whichever strategy owns big blobs wins by construction | `5732f57` (2026-07-29) |
| **D3** | Rest-routing radius = **SHAFT**, not TIP — `RestFieldParams::pencil_radius` was overwritten by `radius()` at all three production call sites | The valley detector was asked where a **3 mm** cylinder can reach, for work the **0.5 mm** tip would do. Every "pencil finds nothing" measurement asked the wrong question | PR-4 `df41169` .. PR-7 `e922931` |
| **D4** | `claims_reference: self_probe` was the shipped default — an **analytic** rest reference that never consults machined stock | In a same-tool cascade the reference is, by construction, exactly what the tool cannot reach, so the rest pass re-cut the whole part. Measured cost when finally compared: 46 366 mm vs 5 259 mm of cutting (**-88.7%**) | `5c24d62` (2026-08-02) |

D1 and D2 interact, and the interaction was itself mis-attributed. The
audit in `unified_v3_design.md` §14t found that **cell size drove the
existence recovery and the dials bought STRUCTURE** (1 blob → 10 regions),
not area — so §14q's "15% recovered by the dial fix alone" credited the
dials with the grid's effect. No experiment crossed the two until that
audit. **Area is not a safe invariant for these fixes**; its direction flips
between fixtures. Region count and topology are the invariants that hold.

---

## 2. The ledger

Origin paths are relative to the repo root. "Defect" cites §1.

### 2.1 The named claims — the ones H4 calls out by name

| # | Claim | Origin | Defect | Re-measured | Result |
|---|---|---|---|---|---|
| 1 | "Contour and pencil have nothing to do on this part" | `planning/unified_v3_design.md:2947-2952`, restated `:1547-1556` | D1, D2, **D3** | **YES (contour half)** | **CONTOUR HALF REFUTED.** On the committed terrain fixture at corrected instruments, waterline owns **41.4% of all cutting** in a single coherent VerySteep region (§3.3 Finding 5) — against §14's **0.0%**. §14's stated basis was *"the fixture has no very-steep band"*; it has one, and it is the second-largest consumer of cutting on the part. **Pencil half NOT tested** — `pencil_claims` is off by design in this comparison (§3.4 limit 5), and an absence under a disabled feature is not evidence. Corroborated independently by the §14t audit on one pinned grid: coarse + shaft dials → **0 VerySteep regions, 0 mm²**; fine + tip dials → **10 regions, 313 mm²**. |
| 2 | "Scallop wins every time" | `planning/unified_v3_design.md:2947-2952`; `FEATURE_CATALOG.md:34` | D1, D2, D3 | **YES, both fixtures** | **NOT REPRODUCED on either.** Concave fixture (§3.2): a split decision — scallop wins the tails (worst overcut **−34 vs −235 µm**) and the ±25 µm bin; the mix arm wins the bulk (p90 **0.2 vs 1.7 µm**) and the clock (−7.8%). Terrain (§3.3): scallop loses p50, p90 **and** worst overcut (**−1 654 vs −528 µm**) and takes **2.94× longer**, keeping only the ±25 µm bin by 0.93 pp. **But read §3.3 Finding 7 first** — on terrain every arm misses the 22.5 µm dial by ~3×, so the µm half of that comparison is inside the noise. The **time** half is not. |
| 3 | The band-mix tables of §14 / §14a / §14m–§14p | `planning/unified_v3_design.md:1547-1556`, `:1616-1642`, `:2347-2399`, `:2492-2559` | D1, D2, D3, D4 | **PARTLY** | Re-measured **as a method**: the mix table now carries a cutting-DISTANCE column, because a move count cannot answer a claim expressed as a share of cutting (§3.1 pins its arithmetic exactly). Honest mixes, by share of cutting: **terrain** scallop 54.9% / waterline 41.4% / raster 3.7%; **groove** raster 79.8% / scallop 20.2%. The §14 tables' *numbers* are not re-derivable — different part, different tool set — and stay void; what is re-earned is that the mix is fixture-dependent and that waterline is a first-class consumer, not a rounding error. |
| 4 | The mm²/s efficiency comparison — **Op B 0.476 vs D 0.938** | `planning/unified_v3_design.md:1577-1583`; restated `planning/v3_campaign_map.md:204-219` | D1, D2, D3 (**and the numerator changed under it — see §2.4**) | **DIRECTION ONLY** | **REVERSED in direction.** Void claim: mix/all-over = 0.51 (half the rate). Re-measured within one run on one instrument: **1.09** on the groove and **2.12** on terrain, and the rest pass alone reads **1.8×** / **4.2×** all-over. But see §3.2 Finding 3 — the rest pass's high rate is ground it re-crossed **without removing anything**, so this is a *motion* measure behaving correctly and inviting the wrong reading. **The absolute numbers 0.476 / 0.938 must never appear beside a post-`a2741a5` figure** (§2.4). |
| 5 | The v3 process-proof closure — "cascade +25% slower at shipped dials" | `planning/v3_workplan.md:267-286` | D1, D2, D3, plausibly D4 | **NO** | **NOT RE-RUN — and deliberately so. See §4.** |

### 2.2 Claims found by grepping planning prose for verdicts citing these instruments' output

These were not named in H4's list. They were found by sweeping `planning/`
and the root docs for verdict-shaped language attached to numbers, as H4's
deliverable clause requires.

| # | Claim | Origin | Defect | Re-measured | Result |
|---|---|---|---|---|---|
| 6 | "§14q: 15% recovered by the dial fix alone" | `planning/unified_v3_design.md:2664-2666` | D1/D2 **mis-attribution** | **N/A** | **Already retracted in place** by `unified_v3_design.md:2864-2866` — the effect belonged to the grid, not the dials. Row kept so the retraction is indexed, not buried. |
| 7 | "313 of 482 mm² recovered — 65%" | `planning/unified_v3_design.md:2789-2799` | measurement-domain error (XY-projected vs 3D-triangle area) | **N/A** | **Already retracted in place** (§14t). Not one of the four defects — a *fifth*, independent instrument error, and the reason `MEASUREMENT_DOMAINS.md` exists. |
| 8 | "The residual is legitimate conditioning" | `planning/unified_v3_design.md:2820` | — | **N/A** | **Already falsified in place** (§14t). |
| 9 | z×1 vs z×2 strategy-mix rebalance ("scallop 80.9% → 55.4%, pencil 5.8% → 21.3%, the mix rebalances as predicted") | `planning/unified_v3_design.md:1616-1642` (§14a) | D1, D2, D3 | **NO** | **NOT RE-RUN** — a *scaled* fixture arm. The scaling premise is downstream of the mix table in row 3; re-running it before row 3 has an answer would measure a rebalance of numbers that are themselves void. Re-open only if row 3's re-measurement supports a mix worth scaling. |
| 10 | "cascade vs D: +25.2% time / +0.2% quality; D scales with slope, cascade is nearly invariant" | `planning/unified_v3_design.md:1656-1685` (§14b) | D1, D2 (cascade side only; D is undecomposed and clean) | **NO** | **NOT RE-RUN** — same reason as row 5, and the section already carries its own contamination note about a `max_rings` defect on the D side. |
| 11 | "The unified finish is not doing rest machining / it ignores the rest / `territory_clip` does not confine it" | `planning/unified_v3_design.md:2347-2399` (§14m) | D4 | **N/A** | **Superseded twice over.** Retracted once in place at `:2422` for a sim-resolution error (0.5 mm cell against a 0.5 mm tool tip), then root-caused at §14p to D4. Both retractions stand; the original claim is void. Indexed here because the *first* retraction was also wrong, which is a fact a future reader needs. |
| 12 | "Contour is dead on this terrain" / "waterline is DEAD at any threshold" | `planning/finishing_stack_review_2026-07.md:1218-1233`; `planning/p2g_quality_matrix_prompt.md:38-56`; `planning/unified_finishing_pass_plan.md:395-411` | D1, D2 | **PARTLY** | **"Dead on this terrain" is REFUTED**: waterline takes **41.4% of cutting** on the committed terrain fixture (§3.3 Finding 5). Whether that work is *worth* its cost is a separate question this wave did not answer — Finding 7 says the fixture cannot adjudicate the quality it buys. **"At ANY threshold" stays NOT RE-RUN**: one threshold pair (45/75) cannot falsify a claim quantified over all of them, and a sweep was out of scope (§3.4 limit 3). |
| 13 | "At the FINE tier the regioned op loses to plain scallop on quality and beats it by only 6% on time; the −20% belongs to the SPEED tier" | `planning/finishing_stack_review_2026-07.md:1218-1233`; `planning/p2g_quality_matrix_prompt.md:38-56` | D1, D2 | **PARTLY** | **The time half survives in shape; the quality half does not.** On fixture 2 at a fine 22.5 µm cusp the regioned op beats all-over scallop by **7.8%** on runtime — close to the "only 6%" the void claim reported, on a different part, which is worth noting. But it does **not** simply "lose on quality": it wins p90 by 8.5×. It loses on the *tails*. "Loses on quality" was too coarse a summary of its own data. |
| 14 | "D (plain scallop) is currently the best fine-quality tool in the shop" | `planning/finishing_stack_review_2026-07.md:1238-1241` | D1, D2 | **YES, on one fixture** | **True in one specific sense, false in another.** All-over scallop is the safest — it left a **−34 µm** worst overcut against the band-mix arm's **−235 µm**, and its deviation map is clean where the other has run-off overcut (§3.2 Finding 2). It is *not* the most accurate in the bulk: p90 1.7 µm vs 0.2 µm. "Best" needs a stated statistic. |
| 15 | "Big-tool→small-tool cascade is REAL for shallows only: mid-steep rest share is 87–96% for every ball Ø2–6" | `planning/p2g_quality_matrix_prompt.md:38-56`; `planning/unified_v3_design.md:59, 239, 581`; `planning/v3_process_proof_prompt.md:73-79`; `planning/unified_finishing_pass_plan.md:395-411` | **instrument clean, conclusion contaminated, fixture unstable** | **NO — and it cannot be** | **NOT RE-RUN, with two caveats that matter.** *(a) The instrument is clean.* The probe (`p2c_headless_ab_wanaka.rs::p2f_ball_rest_share_probe`, verified line by line) uses a **non-tapered** `BallEndmill`, for which `radius() == cusp_radius()`, pins its cell explicitly at 0.05/0.25 mm, and calls `FinishPlannerParams::for_tool(3.0)` — which is the correct cusp radius for a Ø6 ball. D1 and D2 cannot reach any of that. The number is probably sound. *(b) It cannot be re-run anyway.* The probe calls `ProjectSession::load(wanaka_project_path())`, so it reads the live, user-modified play-file this programme is forbidden to depend on — the same reason `wanaka_suggest_baseline` is permanently red. A clean instrument pointed at an unstable fixture, cited four times as load-bearing support for conclusions that ARE contaminated (rows 12–14, the ×2-scale fixture decision, "ball size is not a lever"). Re-earning it needs a committed fixture first. |
| 16 | "Pencil valley-targeting works and is validated: coverage 0.137 → 0.80" | `planning/pencil_investigation_2026-07.md:26-29, 82-84, 208-209, 221-225`; `planning/finishing_stack_review_2026-07.md:797` | **D3** | **NO** | **NOT RE-RUN.** Every number in that investigation was taken on the live wanaka project through the shaft-radius routing field. Re-earning it needs a pencil-specific harness on a committed fixture with real valley structure — not this strategy comparison, which deliberately holds `pencil_claims` off so the only variable is the strategy (§3.4 limit 5). A tracked follow-up, not a finding. |
| 17 | "669 mm cutting vs 9 265 mm with the analytic reference (~14× overestimate)" | `planning/finishing_stack_review_2026-07.md:820-822`; `planning/unified_finishing_pass_plan.md:30`; `planning/unified_v3_design_prompt.md:74-75` | **D3** (magnitudes) and it is the **precursor evidence for D4** | **NO** | **NOT RE-RUN.** Dual status, and both halves need saying. As a *pencil* measurement it is void (D3: the routing radius was the shaft). As *evidence that an analytic reference overestimates rest*, it was **right, and it was ignored for three weeks** — D4 shipped as the default until `5c24d62`. The 14× is the same finding the §14p root cause later measured as 8.8×. Keep the lesson, discard the magnitude. |
| 18 | Catalog row: "−20% vs all-over raster at the SPEED tier; at the fine tier plain Scallop currently wins on quality" | `FEATURE_CATALOG.md:34` | D1, D2 | **MARKED SUPERSEDED** | **It was the only void claim published to a user** — last edited 2026-07-09, three weeks before D1/D2 were fixed, never touched since. The comparative clauses are struck and the row now points here. A tier-by-tier verdict does not belong in a capability catalog at all: it cannot be dated there, and rows 2/13/14 show it was too coarse even when it was current. |
| 19 | "Reference choice is a 30× knob" (712 mm³ self-referenced vs 33 987 / 46 859 mm³ machined) | `planning/rest_cascade_optimal_plan.md:21-40` | earliest documented instance of **D4** | **N/A** | **Vindicated, not superseded.** Written 2026-07-04; D4 shipped as the default for a further four weeks. Indexed so the record shows the defect was described before it was fixed. |
| 20 | `planning/PROGRESS.md` asserts none of this and has absorbed none of the retractions | `planning/PROGRESS.md` | — | **N/A** | **Gap, not a claim.** The master progress ledger carries no reference to any of the four defects or their retractions. Listed so the docs sweep (L1) has it. |

### 2.3 Checked and NOT contaminated — the negative results

Recording these matters as much as the positives: a reader who finds a
verdict not in §2.1/§2.2 needs to know whether it was examined.

* `README.md`, `CLAUDE.md`, `AI_MACHINIST_ANALYSIS_REFERENCE.md` — no
  strategy verdict citing any of the four instruments. `CLAUDE.md:176`
  ("horizontal finish is useless on terrain") concerns a different
  operation family and predates all of this.
* `planning/multitool_finishing_plan.md:8` — "scallop beats raster ~7–21%"
  is a **literature** citation (Lin & Koren 1996; Feng & Li 2002), not a
  measurement from this codebase.
* `planning/STRATEGY_ADVISOR_2026-06-17.md:32` — "spiral wins iff …" is a
  design rule, not a measured verdict.
* `planning/p2g_quality_matrix_prompt.md:232` — "'D wins the fine tier' was
  an instrument artifact" is **already** self-retracted, for a *different*
  instrument (mesh-vertex phase cancellation in the deviation sampler).
  Not attributable to D1–D4.

### 2.4 A change under row 4 that is not a defect, and must not be read as one

Row 4's mm²/s comparison is void for the reasons given, but it is **also
not comparable to any number this wave produces**, for a reason that has
nothing to do with D1–D4.

The §14 table introduces its own numerator, verbatim, as *"**area finished**
per second, which unlike seconds is invariant to tool and stepover"*
(`unified_v3_design.md:1573`).

The metric was re-founded in `a2741a5` as an **honest swept footprint**:
`measurement::swept_footprint_area` rasterises the union of the cutter disc
swept along cutting moves, counting each cell **once**, and
`swept_footprint_mm2_per_s` divides by total runtime. Its own documentation
is emphatic that this is an *emission-stage* measure that **never consults
stock**, and is therefore explicitly **not** "area finished" — the doc goes
out of its way to say it must never be renamed to imply that.

So the rename went in the direction that matters: the old name claimed
**more** than the old number measured. §3.2 Finding 3 shows exactly what
that costs a reader — a rest pass that removed *zero* material scores the
highest mm²/s of any arm, because covering ground and finishing ground are
not the same event.

`MEMORY.md`'s standing rule applies exactly here: *a changed instrument
makes its own docstring a lie you then cite.* **0.476 and 0.938 must never
be printed in the same table as a post-`a2741a5` mm²/s figure.** They are
different measures wearing the same unit, and the older one wore a better
name than it deserved.

---

## 3. The re-measurement

Harness: `crates/rs_cam_core/tests/strategy_comparison_h4.rs`. Metric
contract: §6.2. Fixtures and why: §6.1.

### 3.0 What the harness does and does not do

Three arms, one tool, one stock, one measurement resolution per fixture, so
the only variable is the **strategy**:

| Arm | What it is | The claim it bears on |
|---|---|---|
| **D** | all-over `Scallop` | "scallop wins every time" |
| **B** | `UnifiedFinish` band-mix, shipped `classification_sampler` | the band-mix tables; "contour and pencil have nothing to do" |
| **C** | two-op cascade: all-over `UnifiedFinish`, then `FromRemainingStock` rest pass with `claims_reference: Auto` | the cascade verdicts; the mm²/s comparison |

**The harness asserts no winner.** Its assertions are safety and
non-vacuity: collisions within a stated bound, no resolution clamping, no
empty toolpath, a non-empty column population common to **all three** arms,
Arm B's mix carrying at least two band/strategy combinations, and — for Arm
C — that the rest op is not **rest-blind**. That last one is the A/M6
footgun in gate form: a rest op generated without an intervening simulation
sees no prior stock and silently re-cuts everything, which is exactly how
the original numbers were produced.

A re-measurement that gates on its preferred outcome cannot re-earn a
verdict; it launders the old one through new instruments. The comparison is
printed for a human, and the surfaces are rendered to `target/h4_strategy/`
**before** any assertion runs.

### 3.1 Harness self-check

Committed alongside the evidence runs is a cheap, always-on sentry
(`h4_harness_arithmetic_sentry`) which checks the harness's own arithmetic
rather than any strategy: the per-(band, strategy) cutting distance summed
over the mix table must equal an independently computed cutting distance
over the same move ranges.

```text
H4 sentry: 1 region(s), mix-summed distance 192.056694 mm,
           independent oracle 192.056694 mm
```

Exact agreement. This matters because the mix column is the *new* half of
the instrument — the M3 table carried a move COUNT, which mixes rapids,
links and cuts and therefore cannot answer a claim expressed as a share of
cutting. A harness whose own arithmetic is unchecked is the failure this
whole wave exists to correct.

**A second self-check arrived by accident and is worth keeping.** Fixture 2
was run twice: once on the build at `7d61ad3`, and again after §5.3's
`ScallopReport` split landed. Every reported statistic reproduced to every
digit — p50, p90, both on-size bins, mm²/s, retract trips, collisions and
runtime, on all three arms. That is two facts at once: the split really is
report-only (it moved no toolpath), and this harness is **deterministic
across regenerations**, which the P2.g campaign learned the hard way is not
something to assume — "envelopes equal" once turned out to be cross-run
regeneration variance, and the rule that came out of it was *never compare
across regenerations*. Here the comparison is legitimate because the
reproduction was checked rather than assumed.

### 3.2 Fixture 2 — `grooved_block(2.5, 70°, 1.2)`, the concave arm

7 488 triangles; stock 44 × 28 × 3.2 mm; measurement grid **0.1 mm**
(tip radius 0.5 mm), `resolution_clamped` false on every arm; **96 641
columns, and all three arms share the identical population** — no
intersection loss, so nothing below is comparing different questions.
Shared dials across all arms: cusp target 22.5 µm (`0.3²/(8·0.5)`),
tolerance 0.05 mm, tool Ø1 tip / 7° / Ø6 shank.

| arm | gen s | runtime s | cutting mm | footprint mm² | **mm²/s** | trips | p50 µm | p90 µm | max µm | ±10 µm | ±25 µm | worst overcut µm | worst leftover µm | collisions |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **D** all-over scallop | 18.5 | 105.1 | 3 749.5 | 1 406.0 | 13.3802 | 2 | 0.0 | **1.7** | **174.3** | 94.30% | **97.65%** | **−34.1** | 174.3 | 0 |
| **B** UnifiedFinish mix | 14.4 | **96.9** | 4 276.5 | 1 418.0 | 14.6296 | 10 | 0.0 | **0.2** | 235.0 | 94.07% | 96.93% | −235.0 | 222.9 | 0 |
| **C** cascade (2 ops) | 15.7 | 140.8 | 5 570.6 | 2 486.0 † | 17.6511 † | 58 | 0.0 | **0.2** | 235.0 | 94.07% | 96.93% | −235.0 | 222.9 | 0 |

† whole-arm cascade footprint sums two ops' areas, which double-counts any
ground both crossed — an **upper bound**, not a point estimate, and the
harness says so at the print site.

Arm B's mix (Arm C op0 is identical):

| band | strategy | regions | XY area mm² | cutting mm | share of cutting |
|---|---|---|---|---|---|
| Shallow | raster | 1 | 1 218.1 | 3 411.7 | 79.8% |
| MidSteep | scallop | 1 | 245.7 | 864.8 | 20.2% |

Arm C op1 (the rest pass): Shallow / raster only, 1 region, 619.4 mm²,
**1 294.1 mm of cutting, 48 retract trips, 43.9 s.**

#### Finding 1 — the cascade's rest pass changed nothing. Rendered, not inferred.

Arm C's COLUMNS are **identical to Arm B's in every statistic** — p50, p90,
max, both on-size bins, worst overcut and worst leftover, to every digit
reported. That is the kind of coincidence that is usually an instrument
that failed to update, so it was checked the way this programme's standing
rule requires: `target/h4_strategy/groove_c_minus_b.png` is **uniformly
zero over all 96 641 columns.** Not "small". Zero.

So on this fixture the same-tool rest pass costs **+43.9 s (+45% of the arm's
runtime), 1 294 mm of cutting and 48 retract round trips, and removes no
material at all.**

Two things this does and does not say. It **does** say that a rest pass
which finds nothing still pays full price in motion — the claims pipeline
confined it to 30% of the finish pass's cutting (the non-blindness gate
wanted < 90%, so `claims_reference: Auto` is working), but 30% of nothing is
still 43.9 seconds. It does **not** say cascades are worthless: a same-tool
cascade on a fixture this simple has nothing to rest by construction, and
that is the honest reading. What it removes is any basis for citing a
cascade *quality* benefit on evidence of this shape.

#### Finding 2 — "scallop wins every time" is not reproduced. It is a split decision.

| | D wins | B wins |
|---|---|---|
| tails | max 174.3 vs 235.0 µm; **worst overcut −34.1 vs −235.0 µm** | |
| bulk | ±25 µm 97.65% vs 96.93% | **p90 1.7 vs 0.2 µm** |
| clock | | runtime 96.9 s vs 105.1 s (**−7.8%**) |
| motion | trips 2 vs 10 | |

D is better where the distribution ends; B is better through its body and on
the clock. **B's −235 µm worst overcut is ten times the cusp the dials asked
for**, so the renders were read before this was written down, and they
change the description:

* `groove_b_minus_d.png` — the two arms differ **only inside the groove**.
  Outside it they are pixel-identical, so this is a strategy difference on
  the feature, not a global one.
* `groove_b_dev.png` vs `groove_d_dev.png` — D's groove is a clean, uniform
  blue (leftover cusp) with no overcut anywhere. B's is the same blue field
  **plus red (overcut) cells clustered at the groove's longitudinal ENDS**,
  where the groove runs off the block footprint.

So the overcut is **not** a collar artifact in the middle of the part, which
is what the aggregate alone would have suggested and what I would have
written. It is a **run-off / boundary behaviour** of the banded planner at
the point where a band leaves the stock — narrower in scope, and a different
thing to go and fix. This is the standing rule doing its job: *never gate on
an aggregate without rendering the surface.*

That is a real, actionable difference, and it is exactly the nuance a "wins
every time" claim erases in either direction.

#### Finding 3 — the mm²/s comparison points the other way, and the numbers still may not be compared

Within this run, on one instrument: **B/D = 14.63/13.38 = 1.09** (the mix arm
covers ground 9% *faster* per second), and the rest pass alone reads
**24.32 mm²/s, 1.8× D's rate**. The void claim's ratio was **B/D =
0.476/0.938 = 0.51** — the mix arm at *half* the rate. The direction is
reversed.

Read that as a reversal of *direction only*. Per §2.4 the absolute numbers
are not comparable across the instrument change, and there is a second
reason here: the rest pass's 24.32 mm²/s is high **because it re-crossed
ground it did not cut.** A swept footprint has no notion of "already
finished" — its own documentation is emphatic — so a fast-looking mm²/s on a
pass that removed zero material is the metric behaving correctly and the
reader drawing the wrong conclusion. Footprint throughput is a *motion*
measure. Finding 1 is what says whether the motion was worth anything.

#### Finding 4 — the MECHANISM behind the void claim reproduces, even though its conclusion does not

§14's most-cited line is not the efficiency ratio but the explanation
underneath it: *"Op B pays **0.79** retract round trips per mm² finished; D
pays **0.028** — a **28×** fragmentation gap."*

Re-measured here, on the corrected instruments and a different fixture:

| arm | trips | footprint mm² | trips / mm² | vs all-over |
|---|---|---|---|---|
| D all-over scallop | 2 | 1 406.0 | 0.00142 | 1.0× |
| B band-mix | 10 | 1 418.0 | 0.00705 | 5.0× |
| **C op1 (rest pass)** | **48** | **1 068.0** | **0.0449** | **31.6×** |

The absolute rates are ~18× lower than §14's, as they must be — different
part, different scale, and a footprint denominator rather than an
"area-finished" one. **But the ratio the claim was actually about — a rest
pass fragmenting ~28× worse than an all-over pass — reproduces at 31.6×.**

This is worth separating carefully from row 4. The void claim bundled a
*mechanism* (rest passes fragment, and round-trip COUNT is the cost driver)
with a *conclusion* (therefore the rest pass is half as efficient). The
mechanism survives re-measurement on corrected instruments. The conclusion
does not, and Finding 3 shows why the efficiency ratio was the wrong place
to read the mechanism off: footprint throughput rewards exactly the
ground-covering the fragmentation is spent on.

#### What this fixture cannot decide

**Nothing about contour, waterline or pencil.** The mix contains only
`raster` and `scallop`, and that is by construction, not by discovery: the
groove wall is at 70°, which sits below the 75° waterline threshold, so
waterline territory never existed here. Ledger rows 1 and 12 ("contour and
pencil have nothing to do", "contour is dead") are **untouched by this
fixture** and must not be read as re-earned from it.

That is what fixture 1 is for, and it is the reason the acceptance gate
asked for two.

### 3.3 Fixture 1 — `terrain.stl`, 20 mm window

15 423 triangles, bbox 40.03–59.97 mm in X and Y, 3.0–7.18 mm in Z;
measurement grid **0.1 mm**, no clamping; **38 770 columns common to all
three arms** (D reported 38 771, B and C 38 770 — one column of relevance
difference, intersected away). Same tool and same dials as §3.2.

| arm | gen s | runtime s | cutting mm | footprint mm² | **mm²/s** | trips | p50 µm | p90 µm | max µm | ±10 µm | ±25 µm | worst overcut µm | collisions |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **D** all-over scallop | 44.7 | **462.1** | 2 900.8 | 1 026.0 | 2.2201 | 13 | 67.4 | 291.1 | 2 051.5 | 12.48% | **26.26%** | **−1 654.3** | 0 |
| **B** UnifiedFinish mix | 55.2 | **157.3** | 4 667.4 | 739.0 | 4.6982 | 34 | 62.5 | 265.6 | 2 054.8 | 11.51% | 25.33% | −528.4 | 0 |
| **C** cascade (2 ops) | 75.2 | 232.8 | 5 082.6 | 1 440.0 † | 6.1866 † | 385 | **61.4** | **261.0** | 2 054.8 | 11.75% | 25.75% | −528.4 | 0 |

#### Finding 5 — waterline owns 41% of the cutting. Row 1's stated basis is refuted.

Arm B's mix (Arm C op0 identical):

| band | strategy | regions | XY area mm² | cutting mm | **share of cutting** |
|---|---|---|---|---|---|
| MidSteep | scallop | 1 | 541.9 | 2 560.5 | **54.9%** |
| **VerySteep** | **waterline** | 1 | 394.8 | 1 934.7 | **41.4%** |
| Shallow | raster | 1 | 38.4 | 172.2 | 3.7% |

Set that beside the void table it replaces (`unified_v3_design.md:1547`):

| | void (§14) | re-measured |
|---|---|---|
| scallop | 80.9% | 54.9% |
| raster | 13.3% | 3.7% |
| pencil | 5.8% | — (see below) |
| **waterline** | **0.0%** | **41.4%** |

§14 justified "contour and pencil have nothing to do on this part" with
*"there is no contour work at all — **the fixture has no very-steep
band**"*. On the corrected instruments, on the committed terrain fixture,
the very-steep band is not merely present: it is the **second-largest
consumer of cutting on the part**, and it is a single coherent region, not
a scatter of slivers.

That is what D1 does. A classification grid six times too coarse does not
shrink a very-steep ribbon; it fails to resolve it at all, and a 36×-too-high
area floor then deletes whatever survives. The §14t audit predicted exactly
this on a pinned grid (0 regions coarse → 10 regions fine); this is the same
result reached end-to-end, through generation, with the strategy actually
running on the territory.

**Row 1's contour half is refuted. Its pencil half is not tested here** —
there is no pencil row in this mix, but `pencil_claims` is off in Arms B and
C op0 by design (§3.4 limit 5), so pencil territory was never offered. An
absence under a disabled feature is not evidence.

#### Finding 6 — "scallop wins every time" fails again, and this time not narrowly

All-over scallop is worse than the band-mix arm on **p50, p90 and worst
overcut**, and takes **2.94× longer** (462.1 s vs 157.3 s). Its single
consolation is the ±25 µm bin (26.26% vs 25.33%, +0.93 pp). Its worst
overcut is **−1 654 µm against B's −528 µm** — three times deeper.

The cascade arm is the best of the three on p50 and p90 *and* still finishes
in half of D's time.

**But see Finding 7 before treating any of the µm numbers as a quality
verdict.**

#### Finding 7 — this fixture cannot adjudicate fine quality, and I can now show why

The dials ask for a **22.5 µm** cusp. Every arm's **p50 is 61–67 µm** and
every p90 is **261–291 µm**. All three arms miss the target by roughly
**3× in the median and 12× at p90**, and `terrain_d_dev.png` and
`terrain_b_dev.png` are both dense red-and-blue fields, not the clean cusp
striping the groove fixture produced.

`terrain_b_minus_d.png` says the rest: the two arms differ **everywhere**,
in a fine unstructured speckle, with no spatial organisation at all. That is
cusp-phase noise between two pass patterns laid over a surface whose
**facets are coarser than the cusp being asked for**.

So the 5 µm p50 gap between B and D sits inside a distribution 265 µm wide.
**It is not a quality result.** The time gap (2.94×), the mix (Finding 5) and
the fragmentation counts (Finding 8) are structural and survive; the µm
comparison on this fixture does not.

This independently re-derives §4's objection to the v3 gate — *the fixture
is a coarse TIN, 1.8% of triangles carrying 40.8% of the area* — from a
fresh run rather than from the prior campaign's word. It is also the
strongest single argument for the fixture-quality campaign §4 recommends: on
this mesh, no instrument fix can make a ±10 µm bin mean anything.

#### Finding 8 — the rest pass works HERE, unlike on the groove, and the fragmentation gap holds

Arm C's rest op cut 415.2 mm across all three bands and **did** change the
surface: p50 62.5 → 61.4 µm, p90 265.6 → 261.0. `terrain_c_minus_b.png`
shows scattered red (removed) islands concentrated toward the part boundary
— real rest territory, found and taken. That is the opposite of §3.2
Finding 1, and having both results is the point of running two fixtures: a
same-tool rest pass finds nothing on a simple groove and finds real islands
on relief.

Fragmentation, the mechanism §14 actually identified (cf. §3.2 Finding 4):

| arm | trips | footprint mm² | trips / mm² | vs all-over |
|---|---|---|---|---|
| D all-over scallop | 13 | 1 026.0 | 0.0127 | 1.0× |
| B band-mix | 34 | 739.0 | 0.0460 | 3.6× |
| **C op1 (rest pass)** | **351** | **701.0** | **0.501** | **39.5×** |

31.6× on the groove, **39.5× here**, against §14's 28×. The mechanism
reproduces on both fixtures and on corrected instruments. **It is the most
durable thing the void campaign produced**, and it is worth separating from
the conclusion it was used to support — see §3.2 Finding 3.

### 3.4 Honest limits on this re-measurement

Stated plainly, because a re-measurement that overstates its own reach
recreates the problem it was commissioned to fix.

1. **Debug build, not release.** This wave was disk-constrained and
   instructed not to build `--release`. Every wall-clock number above is a
   DEBUG number. **Runtime seconds are kinematics-integrated machine time,
   not the harness's own wall clock**, so the cross-arm *runtime* comparison
   is unaffected — but the *generation* seconds are debug-speed and mean
   nothing in absolute terms.
2. **Small fixtures.** A 44 × 28 mm block and a 20 mm terrain window. Large
   enough to carry a real band mix and a real column population (96 641 and
   38 770 columns), and far too small to say anything about how any strategy
   scales on a 150 mm part.
3. **One tool, one cusp target, one threshold pair.** 45°/75° and a 22.5 µm
   cusp throughout. Ledger row 12's "at ANY threshold" claim is not
   addressed; a threshold sweep was out of scope.
4. **No live run, no GUI, no machined part.** Every number here is analytic
   or simulated. §6.3(C) is the list of what a human still has to confirm.
5. **`pencil_claims` is off in Arms B and C op0.** The claims pipeline's own
   A/B measured it self-defeating for a single-tool op, and enabling it
   would put a second moving part into a comparison whose only variable must
   be the strategy. It is on in Arm C's rest op, which is where it belongs.
6. **The terrain fixture cannot adjudicate fine quality at all** (§3.3
   Finding 7). Every arm misses the 22.5 µm dial by ~3× in the median, and
   the arms differ from one another in unstructured speckle. The µm columns
   in §3.3 are reported for completeness; the *time*, *mix* and *trip*
   columns are what that fixture can actually support.
7. **The mix table's population comes from the planner's own semantic
   trace** (`SemanticKey::Band` / `Strategy`), i.e. from intent declared at
   the source, not from post-hoc string classification of a move stream.
   That is the right side of the label-vs-geometry rule — but it is worth
   naming, because `arcfit`'s intent inheritance (ledger §6.3 B5) means any
   population selected by *move intent* downstream of arc fitting is still
   unreliable, and this table would not catch that.

---

## 4. Why the v3 closure stays closed

Row 5 reads `NOT RE-RUN`, and this is a decision, not an omission.

The v3 process-proof closed 2026-07-28 with **"NOT PROVABLE on this
fixture"**. H4's own text is explicit that fixing the instruments **does
not resurrect that campaign**, because the closure had two reasons that are
independent of D1–D4 and that both still stand:

1. **The fixture is a coarse TIN.** 1.8% of its triangles carry 40.8% of
   its area. A quality gate on such a mesh is measuring the tessellation as
   much as the toolpath.
2. **The gate bin was below repeatability.** The ±10 µm bin is smaller than
   the run-to-run variation and is **aliased by the 0.25 mm grid**. That
   gate ranked operation D above one that left a **28 mm uncut block** —
   which is the origin of this programme's standing rule: *never gate on an
   aggregate without rendering the surface.*

Neither reason is an instrument defect of the D1–D4 class. Fixing D1–D4
changes what the cascade *does*; it does not make a sub-repeatability,
aliased bin on a coarse TIN capable of adjudicating it.

**What this means going forward.** A negative result that has been
re-earned is different from one that was assumed. This one has *not* been
re-earned — it has been left standing on reasons that were never about the
instruments. Re-opening it is therefore **a fixture-quality campaign**, not
a strategy campaign: it needs a fixture with honest tessellation and a gate
bin above repeatability before the question "does the cascade beat all-over"
can be asked at all. That is a separate, unscheduled work item and it is
recorded here as one.

---

## 5. Intake investigations

Two items arrived from the 2026-07-30 live validation with hypotheses
attached and an explicit instruction: **do not name a mechanism without a
repro.** This subsystem has had five confidently-named mechanisms turn out
not to be the cause. Both are resolved; commit `7d61ad3`.

### 5.1 The chipload gate contradicted its own evidence — hypothesis VERIFIED

**What the operator saw** (`get_toolpath_diagnostics(8)`):
`"Chipload within band (0.0007 mm/tooth)"`, severity `info`, evidence
`min 0.00458 / max 0.00916 / observed 0.000737`, `row_id vendor_lut`,
`extrapolated: true`. Observed is ~6x **below** the stated band minimum and
the verdict is `Within`. Ops 4 and 10, observed 0.012877 against min 0.032
— the same relationship — returned `Exceeds { side: low }`.

**The hypothesis** was *"the gate declines to fail on extrapolated bounds"*,
recorded as a hypothesis and not a conclusion.

**VERIFIED, and it is by design.** F3.3 routes a low-side trip on
weakly-provenanced bounds to a structured `burn_advisory` on the `Within`
arm instead of `Exceeds(Low)`, because a fabricated or stretched burn floor
must not hard-block the operator or the optimizer. The high (breakage) side
stays hard for every provenance. `ChipBoundsSource::low_side_is_advisory`
is the **entire** discriminator between op 8 and ops 4/10;
`tests/chipload_advisory_disclosure_h4.rs` drives it with the observed-vs-min
relationship held fixed and only the bounds source varying.

**The design is not the defect. The report was.** Two report-side defects,
both fixed:

1. The diagnostic adapter's `Within` arm **ignored `burn_advisory`
   entirely** and rendered "Chipload within band". The verdict was carrying
   a structured advisory saying the median sat below the floor; the message
   said the opposite of the evidence stapled to it.
2. `row_id` was **hard-coded to `"vendor_lut"`** regardless of source, which
   is why the citation named a calibrated row and flagged
   `extrapolated: true` in the same breath. `ChipBoundsSource::row_id()`
   now supplies it.

**Deliberately not changed**, and on the Checkpoint E menu instead: the
verdict itself (F3.3's ruling; re-litigating it is not a reporting job), the
diagnostic **id** (the supersession reducer keys on `LOAD_CHIPLOAD_WITHIN`
to silence the pre-sim heuristics — changing it breaks that), and the
`Info` **severity** (raising it moves badge counts, which is a product
decision).

**Not investigated, and still open:** the live session recorded **four**
chipload numbers for that one operation and no two agree — narration
nominal 0.0714, `feeds.chipload_clamped_to_floor` 0.0044 → 0.0250, gate
observed 0.0007, gate band 0.00458–0.00916. This wave verified the
*verdict* mechanism and fixed the *disclosure*. It did **not** reconcile
the four values, and nothing here should be read as having done so.

### 5.2 Peak axial DOC read an exact multiple of the step — mechanism named, with a probe first

**What the operator saw:** op 10 pass 2 read `5.200000762939453` against a
2.6 mm Z step (**exactly 2x**) while pass 1 at the same step read exactly
2.6; op 4 read exactly 3.0 on all five passes against a 3.0 mm step; op 8
read 1.86 mm against a 0.3 mm `z_step` (~6x). Narration offered arc-fit
overshoot, lift bridging or uncleared stock, and **arc-fit had already been
exonerated** on the related Rivers 6.07 mm spike.

**The probe** (`tests/axial_doc_step_multiple_h4.rs`) runs no generator at
all. It hand-builds a toolpath so coverage is known in closed form, drives
the shipped simulator, and reads `axial_engagement_mm` per sample — so
generator, arc fitting, lead-ins, lift bridges, depth-pass planning and
model geometry are absent **by construction** and a reproduction cannot be
attributed to any of them. Three arms at one commanded step:

| arm | ground | measured | ratio |
|---|---|---|---|
| pass 1 | virgin | 2.600000 mm | 1.0000x |
| pass 2 | **cleared** by pass 1 | 2.600000 mm | 1.0000x |
| pass 2 | **virgin** | **5.200000 mm** | **2.0000x** |
| pass 3 | cleared by passes 1 and 2 | 2.600000 mm | 1.0000x |

The **DEEPER** arm is the one the live report did not have, and it is why
this is a discriminating probe rather than a demonstration: it rules out the
reading growing with depth, or with pass index, or with cumulative removal.

**Mechanism.** `peak_axial_doc_mm` is
`max(pre_ray_length - post_ray_length)` over the cells under the cutter's
midpoint disc (`dexel_stock::stamping::stamp_segment_with_metrics`) — the
height of material the stamp **removed**. Nothing in the stamping kernel
knows what the commanded step was, so the number has never been a reading of
one and cannot become one. An exact `n x step` means the column carried `n`
steps of stock when the pass arrived: **a coverage fact about the toolpath,
faithfully measured.**

**So the measurement is sound and the COMPARISON was the defect.**
Narration listed only `DropCutter` as surface-following, so every other
surface op fell through to *"commanded depth_per_pass is unknown"* — and op
8 is a `UnifiedFinish`, whose 0.3 mm `z_step` is the **waterline band's** Z
stepping, not a commanded DOC for the raster and scallop bands that produced
the sample. The "~6x" had no denominator. Fixed: every surface-following op
now says "no commanded DOC", and the advice sentence states what the number
is before offering a cause, with **standing stock named first** and arc-fit
last.

**Which prior conclusions this touches.** Any reading of a peak-axial-DOC
"spike" as evidence of an *emission* defect (arc overshoot, bridging) is
suspect unless upstream coverage was ruled out first. That specifically
includes the open **A/L2 Rivers 6.07 mm spike**, where arc-fit was already
exonerated and no replacement mechanism was ever named — this ledger does
not close it, but it names the first thing to check. It does **not** touch
F-024 (the identity-setup dexel frame defect), which was a genuine frame
error and remains correctly diagnosed.

---

### 5.3 Shipped alongside: `ScallopReport`'s untouched/standing split (M4 sub-decision 5b)

Ruled **yes** at Checkpoint C, outstanding since wave 9b, and the reason it
belongs in this wave rather than another: `uncut_core_mm2` was
**hole-blind** — it summed each truncated cascade polygon's *exterior*
shoelace area and ignored its holes, so an island inside a region the
cascade failed to reach was reported as material left standing when it was
never part of the region at all. A re-measurement wave that shipped a
ledger while leaving a known over-reporting instrument in place would be
missing its own point.

`ScallopReport` now carries, beside the unchanged `uncut_core_mm2`:

* **`untouched_mm2`** — the same truncated-cascade geometry, **hole-aware**
  (exterior minus holes). The M4 oracle's `untouched_mm2` by the same
  definition: area no cutter position ever entered. Exact.
* **`standing_mm2`** — area the cascade *did* ring but where the keep
  predicate dropped every point, so no cut landed. The oracle's "reached but
  left high", in the cascade's own terms. **An estimator**, not an area:
  arc length owned by dropped ring points × the offset stepover that
  produced the ring.

Each carries its own `MeasurementProvenance` rather than sharing
`PROVENANCE`, because they do not share its resolution statement, and the
`standing` one names what it cannot distinguish (off-part geometry vs a
genuine left-high residual). Both reach `ToolpathStats` through A/M9's
channel under the same **three-valued** contract — `None` is *not measured*,
`Some(0.0)` is *measured and clean* — and both are report-only: no gate
consumes them, no verdict moves.

The sentry (`tests/scallop_untouched_standing_h4.rs`) is built around the
assertion that proves the change did something rather than decorating:
on a truncated region containing a hole of known closed-form area,
`untouched_mm2` must fall short of `uncut_core_mm2` by that area. Without
that test the split would be two new fields agreeing with the old one.

---

## 6. Checkpoint E — the decision menu

Addendum B §B.3 says the orchestrator stops here for human review before any
H4 re-run is published. This section is that stop.

### 6.0 Readiness, as §B.3 requires it stated

| Prerequisite | State |
|---|---|
| **H2.1** rest-routing radius | **LANDED** — PR-4 `df41169` … PR-7 `e922931` |
| **A/M6** `claims_reference` default | **LANDED** — `5c24d62`, now `Auto` with its derivation recorded |
| **Checkpoint B** generation-grid decision | **DECIDED** — the quality half of every speed/quality trade is pinned |
| Fixture choice, and why wanaka alone is insufficient | §6.1 |
| Metric contract per M1 — domain, stage, resolution, both sides of every ratio | §6.2 |
| Rendered surfaces accompanying every aggregate | §3, `target/h4_strategy/` |

### 6.1 The fixture choice, and why wanaka alone will not do

H4's acceptance gate is explicit: **"Do not re-run on wanaka alone — it
defeated the v3 gate for reasons unrelated to strategy choice."**
Independently, `planning/airrun_2026-06-01/wanaka.toml` is a live,
user-modified play-file that this programme is forbidden to read as a gate
dependency; `wanaka_suggest_baseline` is permanently red for exactly that
reason — a verdict that changes when somebody drags a slider is not a
verdict.

So the re-runs use two **committed** fixtures:

1. **`tests/fixtures/terrain.stl`**, windowed — the wanaka-class relief the
   M3 study measured, with real concave valleys and a full slope spectrum.
   It is representative, and it is a coarse TIN, which is precisely the
   §4 objection: it can show what the strategies *do*, but its own
   tessellation limits what a quality bin can *adjudicate*.
2. **`common::meshes::grooved_block`** — a groove cut INTO a flat block:
   flat rim, straight walls at a chosen angle, flat floor. Genuinely
   **concave** and with **homogeneous, separable** shallow and steep spans.
   This is the fixture H4 demands ("at least one fixture with genuinely
   separable steep/shallow territory") and it exists because of P12's
   lesson: **a convex fixture cannot adjudicate a concave defect.** Wave
   14 found that same blind spot in a third place — the one benchmark shape
   with no reflex corners was the one shape the defect could never appear
   on.

Neither fixture makes the v3 question answerable (§4). Together they make
the *strategy-mix* question answerable, which is a smaller and different
question.

### 6.2 The metric contract (M1)

Every ratio below states both sides. This is not ceremony: row 4 of the
ledger is void partly because a *changed* instrument kept its old name.

| Measure | Domain | Stage | Resolution |
|---|---|---|---|
| **COLUMNS quality** | per-dexel-column signed deviation from the reference model, µm | **Simulation** | sim cell, pinned well below the tool **TIP** radius; `resolution_clamped` asserted false, and the population is the **intersection across all arms** |
| **mm²/s numerator** | `ProjectedXyAreaMm2` — union of the cutter disc swept along **cutting** moves, each cell counted once | **Emission** | explicit footprint cell; **never consults stock**, so it is throughput of *footprint*, not of *material removed* |
| **mm²/s denominator** | seconds of **total** runtime (cutting + rapids) | kinematics-integrated | machine profile |
| **Retract trips** | count of maximal contiguous `Rapid` runs; `in_node`/`between_nodes` split only when `spans_valid` | **Emission** | move stream; `None` where unmeasured, never a fabricated 0 |
| **Mix table** | per-(band, strategy) region count, **cutting distance mm**, XY-projected area | **Emission**, from the planner's own semantic trace | — |

Two rules are being obeyed here rather than restated:

* **Both sides of a ratio must be the same measure.** `MEMORY.md`'s
  instrument-integrity entry: projected area over 3D area is how §14r's
  "65% recovered" became a number that was not a measurement.
* **A move COUNT is not a cutting measure.** The mix tables being re-earned
  were expressed as shares *of cutting*, so the mix table carries a
  distance column and not only the move count the M3 harness printed.

### 6.3 What the operator is being asked to decide

**A. Promotion into planning prose and the catalog.**

| # | Question | Recommendation |
|---|---|---|
| A1 | `FEATURE_CATALOG.md:34` is the only **user-facing** void claim (ledger row 18), last edited 2026-07-09 and never revised through four instrument fixes. Replace with re-measured text, or strike the comparative clauses and leave the capability description? | **Strike the comparative clauses now, regardless of what §3 says.** A shipped catalog should not carry a tier-by-tier verdict at all; that belongs in planning prose where it can be dated. |
| A2 | Should the re-earned §3 verdicts be written back into `unified_v3_design.md` / `finishing_stack_review_2026-07.md` in place, or should those documents be left as dated records with a pointer to this ledger? | **Pointer, not rewrite.** Those files are the record of how the programme reasoned, errors included. Rewriting them destroys the audit trail that made D1–D4 findable. Add a banner at the top of each pointing here. |
| A3 | `planning/PROGRESS.md` has absorbed **none** of the retractions (row 20). Should it carry a one-line "strategy verdicts are superseded, see the ledger"? | **Yes** — it is the file a new session reads first. |
| A4 | §3.2 Finding 1: a same-tool cascade rest pass costs **+45% runtime, 1 294 mm of cutting and 48 retract trips** to remove **zero** material (rendered). Should the planner **warn** when a rest pass's claimed territory produces no removal against its reference? | **Yes, as a report**, and it is cheap: the claims pipeline already knows the reference and the territory. This is A/M9's channel again — a finding with no home on the toolpath. Note it is *not* a refusal: the operator may legitimately want the pass. |
| A5 | §3.2 Finding 2: Arm B leaves a **−235 µm overcut where a band runs off the stock footprint**, ten times the 22.5 µm cusp; all-over scallop leaves −34 µm on the same fixture. Is that a defect to file? | **Yes, file it** — it is localised, reproducible on a committed fixture, and now rendered. It is a *boundary* behaviour, not the collar the aggregate implied. |
| A6 | **`ToolpathStats::standing_material_mm2` is misnamed.** §5.3 gave the cascade the oracle's vocabulary, and doing so exposed that the *existing* field measures what the oracle calls **untouched** (never reached — it is the truncated cascade core) while wearing the word **standing** (reached, left high). Rename it to something like `truncated_core_mm2`? | **Operator's call, because it is not free.** The field is load-bearing across serde project files, the GUI and MCP, so a rename is a compatibility event, not a doc fix. What this wave did instead: named the conflict in the field docs, and refused to compound it — the new field was briefly called `standing_material_estimate_mm2`, which reads as "an estimate of `standing_material_mm2`" and is false, and it is now `reached_uncut_estimate_mm2`. A wave about names that lie should not ship one. |

**B. What stays open.**

| # | Item | Why it stays open |
|---|---|---|
| B1 | The **v3 closure** (row 5) | §4. Re-opening is a *fixture-quality* campaign — honest tessellation and a gate bin above repeatability — not a strategy campaign. Unscheduled. |
| B2 | Rows 9, 10 (scaled-fixture and cascade-invariance arms) | Downstream of §3. Re-open only if §3 supports a mix worth scaling. |
| B3 | The **four disagreeing chipload numbers** (§5.1) | This wave verified the *verdict* mechanism and fixed the *disclosure*. It did not reconcile narration-nominal 0.0714 / clamped 0.0044→0.0250 / gate-observed 0.0007 / band 0.00458–0.00916. |
| B4 | **A/L2, the Rivers 6.07 mm axial spike** | §5.2 names the first thing to check (upstream coverage) and rules out the mechanism that was assumed. It does not close it. |
| B5 | `arcfit`'s **intent inheritance** | Named as a real defect left standing since wave 12. It relabels lead/entry geometry as cutting, so *every gate that selects a population by label* reads it wrong. Fixing it re-pins fingerprints repo-wide. A wave, not a footnote. |
| B6 | Chipload diagnostic **severity** (§5.1) | Left at `Info` on purpose. Raising it for the advisory case moves badge counts — a product decision. |
| B7 | The **GUI worker's hand-copied findings path** | Found during §5.3's field audit: `rs_cam_viz/src/compute/worker/execute/mod.rs` copies `GenerationFindings` onto GUI-side `ToolpathStats` field by field, in parallel with the session path. The two new fields needed the same copy lines or they would have compiled cleanly and stayed `None` in the live GUI forever. That is the *fourth* time this shape of duplication has silently dropped side-data. It was patched, not fixed — the duplication is still there. |
| B8 | `GEOM_STANDING_MATERIAL` and the MCP per-toolpath summary do **not** carry the §5.3 split | Narration does. The diagnostic message and `ToolpathDiagnostic` were time-boxed out. Small, and worth doing when someone is next in that file. |

**C. What live validation must confirm.** None of this wave is live; every
number is headless and analytic. Addendum B §B.4's prerequisites apply in
full (rebuild release BEFORE connecting — `.mcp.json` runs `cargo run
--release` and a cold build blows the 30 s connect timeout; restore session
state; **match sim resolution to the tool TIP**; regenerate rest ops after
re-simulating, because a new simulation does **not** mark toolpaths stale;
never save over wanaka).

| # | Must confirm | Origin |
|---|---|---|
| C1 | **Hookup-default regeneration on an OLD project.** `intra_pass_hookup_mm` is `#[serde(default)]` at 3.0 and `intra_region_hookup_mm` ships ON at 6.0, so a saved project without the keys reloads with links and regenerates. The operator's Checkpoint D ruling carried this re-check as an explicit condition. | waves 12, 14 |
| C2 | **TSP-reorder value on a real part.** Surface ops now reorder behind region-node barriers, and the span-tiling defect that inverted the mix table is fixed (`77f2b7a`). Never seen on a part. | wave 12 / span defect |
| C3 | **Per-point fan geometry** (C9 claims fan) — measured pointwise, never rendered. | wave 10 |
| C4 | **`claims_reference: Auto`** picks `machined_stock` in a real cascade and says so. This is D4's fix; if Auto mis-derives on a live chain, every cascade number regresses to void. | A/M6, `5c24d62` |
| C5 | **M3's +5.6% cutting time** on wanaka-class relief. Quality is a wash and generation is 22.5% faster, but the honest classifier finds more very-steep ground and waterline is the expensive strategy. The lever if unwanted is `waterline_threshold_deg`, **not** the classifier. | wave 7b |
| C6 | **Collision count stability across simulation resolutions** on a fixed toolpath (definition-of-done item 15). The live 20-collision reading was a 0.1 vs 0.5 mm resolution asymmetry, not an emission defect. **Never clear collisions across mismatched resolutions.** | wave 11 / A/M10 |
| C7 | The **narration and chipload wording changes** of §5 read correctly on a real diagnostic panel. They are the only operator-visible output this wave moved. | this wave |
