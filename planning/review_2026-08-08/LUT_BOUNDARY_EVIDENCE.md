# A-6 — LUT selection delta, chipload boundary contract, G-CHIP-ULP

**Wave**: TD3 A-6 (research). **Date**: 2026-08-13. **Branch**:
`tech-debt-3`, parent `e66962ae`, instruments at `3ac55fcf`. Dev
profile, no release build.

**Ledger rows**: `F-LUT2`, `G-CHIP-ULP` (`TECH_DEBT_2_CLOSEOUT.md` §4);
intake `G-LIT-IPE`, `G-SUB1MM`.

**Nothing in this wave changes behaviour.** Two instruments were
committed (`3ac55fcf`), both number-preserving; the recipe numbers wait
on Checkpoint K.

Artifacts: `artifacts/a6/lut_resolver_census.txt`,
`artifacts/a6/g_chip_ulp_riders.txt`,
`artifacts/a6/literature_matrix_full.txt`.

Reproduce:

```text
cargo test -p rs_cam_core --test lut_resolver_census_a6 -- --ignored --nocapture
cargo test -p rs_cam_core --test chipload_boundary_g_chip_ulp -- --nocapture
```

---

## 0. Headline

1. **F-LUT2 names the smaller of two axes.** The two entry points
   (`find_best_row_for_geometry` vs `find_best_chip_envelope_row`)
   diverge on **141 of 18 144** swept queries — **3 of 48**
   (family, geometry-class) cells, 0.78 %. Every divergence is one
   class: Suggest matched an RPM-only row and fell back to the formula
   chipload. **No cell has both sides banded.**
2. **A-5's 1.273× is not the entry point.** It is the operation-family
   reroute `routed_lookup_family`, which the gate applies and Suggest
   does not. On that axis **489 of 3 024** pairs resolve to different
   rows, band maximum ratio **×0.16 … ×6.17**, plus **756 gate refusals**
   of which **378** leave Suggest holding a banded recommendation on a
   surface where the gate declined to judge at all.
3. **G-CHIP-ULP reproduces in full**, and one of its four riders is
   mis-stated in the ledger: **every shipped gate is `Within` at exact
   equality.** The live `Exceeds` was the float round trip, not a
   boundary-semantics disagreement.
4. **The band a verdict is compared against moves 14.16 % with the
   simulation cell** on the reference cutter, and W10-LV's own DOC pair
   flips one identical feed `Within` → `Exceeds(High)`.
5. **G-SUB1MM is a stale expected value**, not a wrong law. **G-LIT-IPE
   is a genuine literature-vs-LUT conflict** with an *empty* feasible
   set — no feed satisfies both the cell and the matched vendor band.

---

## 1. F-LUT2 axis 1 — the two entry points (the census the row asks for)

### 1.1 The structural relation

`find_best_chip_envelope_row` is `find_best_row_for_geometry` with one
extra candidate **filter**: `obs.chipload_min_mm_tooth.is_some() ||
obs.chipload_max_mm_tooth.is_some()`
(`crates/rs_cam_core/src/feeds/vendor_lookup.rs:242-261`). Same scorer,
same tie-break, same angle-aware dispatch. The envelope candidate set is
therefore a **strict subset** of the geometry candidate set.

Consequences, and they are the whole taxonomy:

- the envelope resolver can never match where the geometry resolver does
  not (pinned by `envelope_resolver_never_matches_where_geometry_resolver_does_not`);
- they differ **only** when an RPM-only row outscores every
  chipload-bearing row.

### 1.2 The sweep

8 op families × 6 geometry classes × 3 pass roles × 7 materials ×
6 diameters × 3 flute counts = **18 144 queries** against the shipped
252-row LUT.

| outcome | count | share |
|---|---:|---:|
| `BothNone` — no LUT coverage for the query at all | 13 653 | 75.25 % |
| `Same` — both resolvers, same row | 4 350 | 23.97 % |
| `DifferentSuggestUnbanded` — Suggest on an RPM-only row, gate on a banded one | **141** | **0.78 %** |
| `GateBlind` — Suggest matched, gate found nothing | 0 | 0 % |
| `DifferentBothBanded` — two different *bands* | **0** | **0 %** |

**3 of 48 cells diverge**, all on the same axis:

| op family | geometry class | divergent | of |
|---|---|---:|---:|
| Adaptive | Flat | 9 | 378 |
| Adaptive | Bull | 6 | 378 |
| Trace | VBit60 | 126 | 378 |

### 1.3 What the divergence *costs*

In every one of the 141 cases the geometry resolver's row publishes no
chipload column, so `feeds/mod.rs:1068` (`result.chip_load_mm > 0.0`)
is false and Suggest:

- takes `formula_chipload` instead of a vendor number, and
- sets **`chipload_bounds: None`** — no band at all.

So the operator-visible consequence is *not* "two numeric bands", it is
"Suggest has no band and the gate has one". Downstream that means the
Step-9b rubbing-floor clamp falls back to the bare
`RUBBING_FLOOR_MM_TOOTH` (no subordination — `effective_rubbing_floor`'s
`None` arm), `SuggestAggressiveness::target_chipload` has nothing to
aim at, and the feeds modal shows a formula number beside a gate verdict
computed from a vendor row.

**The 141 are also the cases where the RPM anchor is doing real work.**
That is the standing operator preference in the other direction
(memory: `lut-hardness-agnostic` — material/hardness should *dial*
params, not hard-reject rows): the RPM-only rows exist deliberately as
feeds-calculator anchors. Unifying on the envelope resolver would take
the RPM anchor away from Suggest on those 141; unifying on the geometry
resolver would hand the gate a row with no envelope and force
`Unmodeled(NoVendorData)` where it currently judges. **Neither
single-resolver answer is free**, which is the substance of question (a).

---

## 2. F-LUT2 axis 2 — the operation-family reroute (the larger half)

### 2.1 The mechanism, and the re-attribution of A-5's 1.273×

The two consumers do not build the same **query**:

| | family the query names |
|---|---|
| Suggest | `vendor_normalize::op_family_to_lut(input.operation)` — straight 1:1 off `feeds_family` |
| gate / optimizer / viewport | the same value passed through `tool_load::chipload::routed_lookup_family` |

`routed_lookup_family` (`chipload.rs:1023-1044`) reroutes two operation
kinds and nothing applies it on the Suggest side:

| operation | Suggest queries | gate queries |
|---|---|---|
| `Adaptive3d` | `Adaptive` | **`Pocket`** (role unchanged) |
| `ProjectCurve`, ball / tapered ball | `Trace` (op's role) | **`Parallel` + `Finish` forced** |
| `ProjectCurve`, flat | `Trace` (op's role) | **`Contour` + `Finish` forced** |
| `ProjectCurve`, bull nose / V-bit / facing | `Trace` | **refusal** → `Unmodeled(NoVendorData)` |

A-5 recorded its 1.273× under F-LUT2. **It is this, not the entry
point** — proven two ways in
`a5_band_divergence_is_the_reroute_not_the_entry_point`:

1. On A-5's A3D-1 query (Ø6 2F flat, hard maple Janka 1450, roughing)
   **both entry points return the same row for both families**. The
   resolver is exonerated on this cell.
2. The two bands differ in **shape** — `max/min` **1.842** (Adaptive
   row) vs **1.719** (Pocket row) — and no scale factor (diameter law,
   hardness law, DOC derate) can change a ratio. Two rows, reached from
   two families:

```text
Suggest: amana-flat-hardwood-adaptive-6000-2f   0.038  – 0.070
gate:    amana-flat-hardwood-pocket-6000-2f     0.032  – 0.055
                                                        ×1.2727
```

A-5's A3D-2 pair reproduces the identical ×1.2727 because it is the
same row pair carried by the same scale factors.

### 2.2 The reroute census

3 024 (query, reroute) pairs swept:

| | count |
|---|---:|
| different rows | **489** |
| gate refusals (`ProjectCurve` on bull / V-bit) | **756** |
| …of which Suggest still returns a **banded** recommendation | **378** |

Band-maximum ratio (Suggest ÷ gate) over the 489: **min ×0.1602,
median ×0.9798, max ×6.1658.**

Representative Ø6 2-flute roughing rows:

| reroute | geom | material | Suggest row (max) | gate row (max) | ratio |
|---|---|---|---|---|---:|
| Adaptive3d | Flat | oak | `amana-flat-hardwood-adaptive-6000-2f` (0.07421) | `amana-flat-hardwood-pocket-6000-2f` (0.05831) | **×1.2727** |
| Adaptive3d | Flat | pine | `…softwood-adaptive…` (0.10258) | `…softwood-pocket…` (0.07926) | ×1.2941 |
| Adaptive3d | Flat | mdf | `…hardwood-adaptive…` (0.08885) | `amana-flat-mdf-pocket-6000-2f` (0.06080) | **×1.4612** |
| Adaptive3d | Flat | ply-birch | `…hardwood-adaptive…` (0.07663) | `…plywood-hardwood-pocket…` (0.05455) | ×1.4049 |
| Adaptive3d | Flat | alu6061 | `…aluminum-adaptive-6000-3f` (0.03500) | `amana-zrn-flat-aluminum-pocket…` (0.15240) | **×0.2297** |
| Adaptive3d | Bull | oak | `onsrud-bull-hardwood-adaptive…` (0.06361) | `amana-flat-hardwood-pocket…` (0.05831) | ×1.0909 |

Two directions, two different failure modes:

- **ratio > 1** — Suggest recommends against a **wider** band than the
  gate judges with. This is the direction that lets a pre-simulation
  solve aim past a ceiling it cannot see. A-5 measured its practical
  consequence: arm C ("re-key to 1.0") cleared the gate with a **1.8 %**
  margin, by luck of where the two bands sit; under
  `SuggestAggressiveness::Speed`, which targets the band maximum, the
  same solve aims **1.27× over** the gate's maximum.
- **ratio < 1** (down to ×0.16) — Suggest is more conservative than the
  gate. Not a safety hazard; it is a silent throughput loss with no
  surface that explains it.

### 2.3 The refusal class is the sharpest one

378 pairs: `ProjectCurve` on a bull-nose or V-bit cutter. Suggest
returns a vendor-backed band; the gate returns
`Unmodeled(NoVendorData)`. The operator sees a confident number and no
verdict, and nothing on any surface says the two are talking about
different rows — or that the gate declined because of the *cutter class*
rather than because the data is missing.

### 2.4 One instrument-integrity note

The existing parity test `tests/lookup_parity.rs` calls
**`find_best_row` on both sides** and hand-mirrors the gate's query. It
therefore cannot see either axis: not the entry-point difference (it
uses neither entry point) and not the reroute (its `gate_op_family`
field is supplied by the case, and its five cases are all
non-rerouted operations). Its header even says the `project_curve`
divergence is "a feature, not a parity bug" and excludes it. That
exclusion is the second axis, unmeasured, since Item B.

---

## 3. G-CHIP-ULP — the four riders, pinned

Fixture: `crates/rs_cam_core/tests/chipload_boundary_g_chip_ulp.rs`
(6 tests, all green — they pin the **current, defective** behaviour, and
each carries the retirement instruction for when Checkpoint K lands).

### 3.1 Rider 1 — the floor collapses onto the CEILING

`effective_rubbing_floor(band) = min(RUBBING_FLOOR_MM_TOOTH,
derated_band_max)`. On the B3 reference (Ø1 tapered ball, 2 flutes,
5.26°, scallop finish, hard maple):

```text
derated band        0.005762689177314920 .. 0.011525378354629830
global floor        0.025
effective floor     0.011525378354629830   == band.MAX exactly
Suggest recipe      rpm 18500, feed 426.438999, fpt 0.011525378354629830
warning             ChiploadClampedToFloor { band_capped_from: Some(0.025) }
```

The recipe is parked **exactly on the breakage-side bound**, with zero
headroom, on the exact quantity the gate compares. This is not a bug in
any single step — the 2026-08-06 ruling that subordinated the floor to
the band was correct, and the alternative (clamping *up* to 0.025) was
3.47× the band maximum. The consequence is that the boundary comparison
became load-bearing.

### 3.2 Rider 2 — the observation is a float round trip, so the side is noise

Suggest **multiplies**: `feed = fpt × rpm × flutes`
(`feeds/mod.rs:1561`). The gate **divides**:
`observed = effective_feed / (rpm × flutes)` (`chipload.rs:162-168`).
`(x·a)/a ≠ x` in binary floating point.

Round-trip census over a 291-value RPM grid × 4 flute counts:

| band maximum | lands **above** | exact | below |
|---|---:|---:|---:|
| ledger's wanaka value `0.01278107701207316` | **89 / 1164 (7.6 %)** | 994 | 81 |
| live B3 `0.01152537835462983` | 72 / 1164 (6.2 %) | 1043 | 49 |

**Through the real gate**, with the feed parked exactly on the band
ceiling the gate itself reports:

```text
Exceeds(High) at 12 of 181 RPM values (6 000 … 24 000, 2 flutes)
Within        at 169
every trip:  delta = 1.734723475976807e-18   ( = 1 ulp )
printed:     observed 0.011525  vs  max 0.011525
```

The verdict is decided by the rounding of a multiply/divide pair. The
comparison is `observed > max × (1 + tolerance.breakage)` and
`ToleranceBands::default()` is all-zeros, so there is no epsilon to
absorb it.

### 3.3 Rider 2b — the band itself is resolution-dependent

The gate queries the LUT at `tool.lookup_diameter_at(peak steady-state
axial DOC)` and derates by `peak_doc ÷ that diameter`
(`chipload.rs:521-536`). **Peak axial DOC is a dexel measurement**, so
the simulation cell moves it, and on any non-cylindrical cutter it moves
the queried diameter with it. Two terms then move the band — the `D^0.61`
diameter law and the piecewise DOC derate — **in opposite directions**,
so the net is small, signed, and not predictable from either law alone:

| peak axial DOC (mm) | queried Ø | band min | band max | Δ vs first |
|---:|---:|---:|---:|---:|
| 0.350 | 0.9539 | 0.005763 | 0.011525 | — |
| 0.700 | 1.0411 | 0.006078 | 0.012156 | +5.48 % |
| 1.050 | 1.1055 | 0.006305 | 0.012610 | **+9.41 %** |
| 1.673 | 1.2202 | 0.006075 | 0.012150 | +5.42 % |
| 2.020 | 1.2841 | 0.005918 | 0.011837 | +2.70 % |
| 3.000 | 1.4645 | 0.005523 | 0.011046 | **−4.16 %** |

**Total span 14.16 %** in the bound a verdict is compared against.
W10-LV's own pair (queried Ø 1.308 → 1.372 between 1.0 mm and 0.1 mm
cells; here Ø 1.2202 → 1.2841 on the reference cutter) moves the band
**−2.58 %**, and one identical feed of 437.4140 mm/min reads:

```text
coarse-DOC reading:  Within
fine-DOC reading:    Exceeds(High)
```

exactly the ledger's "coarse read Within at 97.8 %, fine read Exceeds".
**Record the cell beside every chipload verdict**, as the programme
already requires for collision counts.

### 3.4 Rider 3 — the clamp has no name in the binding vocabulary

`ModulationSummary::binding_constraint_distribution` is the surface that
answers "why is the feed here". Its vocabulary is
`tool_load::BindingConstraint`: `ChiploadMax`, `ChiploadMin`,
`DeflectionMax`, `PowerMax`, `MachineMaxFeed`, `KinematicReach`.
**None denotes Suggest's rubbing-floor clamp**, which is what parked the
feed in this whole scenario.

`ChiploadMin` is the nearest name and is a **different quantity**:

| | quantity | value on B3 |
|---|---|---:|
| `BindingConstraint::ChiploadMin` (`feed_modulation.rs:470`) | `band.min × rpm × flutes` | fpt 0.005763 |
| Suggest's Step-9b clamp (`feeds/mod.rs:1552`) | `effective_rubbing_floor` = `band.max` here | fpt 0.011525 |

**Ratio 2.000× — they point at opposite ends of the band.** And
`ChiploadMin`'s own doc comment calls it "the rubbing floor", which is
rule 5's failure mode: a docstring that names a constant the code does
not use. Correcting that wording is free and independent of (d).

### 3.5 Rider 4 — the cross-gate boundary contract, measured

The ledger records the semantics as *disagreeing* (drill `plunge_feed`
observed==lower bound ⇒ `Within`, chipload observed==upper bound ⇒
`Exceeds`). **Measured, that is not what the code does.**

| gate | side | comparison | at **exact** equality | epsilon dial | observation path |
|---|---|---|---|---|---|
| chipload | high | `observed > max × (1 + breakage)` | **Within** | `ToleranceBands::breakage`, default **0.0** | **multiply→divide round trip** |
| chipload | high, **+1 ulp** | same | **Exceeds(High)** | — | — |
| chipload | low | `median < min × (1 − burn)` | **Within** | `ToleranceBands::burn`, default 0.0 (+ `low_side_is_advisory` demotion) | same round trip |
| power | high | `peak > available × (1 + power_breach)` | **Within** | `power_breach`, default 0.0 | integrated |
| deflection | high | `peak > EXCEEDS_BOUND_MM × (1 + deflection_breach)` | **Within** | `deflection_breach`, default 0.0 | integrated |
| drill plunge feed | low | `observed < lo` | **Within** | **none** | single division |
| drill plunge feed | high | `observed > hi` | **Within** | **none** | single division |
| drill peck adequacy | high | `observed <= threshold` | **Within** | **none** | direct |
| drill chip welding | banded | `Low [0, 0.75t)` / `Elevated [0.75t, t)` / `High [t, ∞)` | **`Elevated`** — an `Exceeds` variant *below* the threshold it names | **none** | direct |

**Every gate is `Within` at exact equality.** The ledger's
"boundary semantics disagree" reading is a mis-attribution of rider 2 —
the live chipload observation was 1 ulp *above* the bound, not equal to
it. What genuinely differs across the gates is:

1. **the drill gates carry no epsilon dial at all**, while the three
   milling gates each carry one that defaults to zero; and
2. **only the chipload observation reaches its bound through a
   multiply/divide round trip**, so only it can land 1 ulp off a value
   the operator set exactly.

This matters for (b): a contract written as "make the comparisons agree"
solves nothing, because they already agree. The contract has to be about
the **epsilon** and the **reconstruction**.

---

## 4. G-LIT-IPE — diagnosis

**Cell** `flat_3mm_pocket_ipe_extreme`
(`tests/literature_matrix/cells.toml:4957`): Ø3 2-flute flat, pocket,
Ipe (Janka 3510), Shapeoko XXL. Red **at master itself** — not a TD3
regression.

Runner output (`artifacts/a6/literature_matrix_full.txt`):

```text
rpm 22000   feed 1585.0   fpt 0.0360 mm/tooth   doc 0.600   woc 1.050
  rpm                                      Within   22000 ∈ [18000, 22000] (at edge)
  fpt                                      Outside  0.0360 > max 0.0270 (+33.4 %)
  axial_doc                                Outside  0.6000 > max 0.5000 (+20.0 %)
  plunge/feed                              Outside  0.1167 < min 0.3000 (−61.1 %)
  anti.ipe_micro_matches_oak_micro_chipload  Outside  `fpt > 0.030` triggered
  verdict: critical
```

**Three independent rows are Outside; only the anti-test is critical.**

### 4.1 Is the anti-test's premise right?

Its name says the failure is "Ipe micro-tool got the same chipload as an
oak micro-tool". Measured at Ø3 2F pocket roughing through
`feeds::calculate`:

| material | matched row | scaled band | commanded fpt |
|---|---|---|---:|
| Oak (Janka 1290) Ø3 | `amana-zrn-flat-mdf-pocket-3175-2f` | 0.0662 – 0.1103 | **0.0740** |
| Ipe (Janka 3510) Ø3 | `freud-solid-carbide-eighth-hardwood` | 0.0297 – 0.0744 | **0.0437** |

The Ipe recommendation is **0.59× the oak one** — the `Janka^-0.5` law
*is* firing (`chipload_hardness_scale` 0.6062). **The premise the
anti-test's name states is false at the current revision**: Ipe does not
match oak. What the anti-test actually measures is an **absolute** bar,
`fpt > 0.030`, and the recommendation clears it because the shipped Ø3
hardwood rows are far more aggressive in absolute terms than the cell's
literature band.

### 4.2 The conflict is structural, and the feasible set is empty

- cell's expected fpt band: **0.013 – 0.027** (sources: `onsrud_hwood`,
  `gwizard_hwood`, `fpl_wood_handbook`)
- matched vendor band after both scaling laws: **0.0297 – 0.0744**

**These do not intersect.** The vendor band's *minimum* is above the
literature band's *maximum*. And the anti-test fires above 0.030, so the
only fpt satisfying both the matched band and the anti-test is
`[0.0297, 0.030]` — a **1 % sliver of a 150 %-wide band**, reachable
only by accident. No tuning of the two scaling laws closes a gap between
a band's floor and another band's ceiling.

So this is **not** a scaling-law defect and **not** a stale expected
value in the ordinary sense. It is a **literature-vs-shipped-LUT
disagreement about Ø3 hardwood chipload**, roughly 2× wide, that the
cell is correctly detecting and that no side of the engine can resolve
on its own.

### 4.3 Note on the `D^0.61` law here

The diameter scale on the Ipe match is 0.9660 — the matched Freud row is
calibrated at 1/8″ (3.175 mm) against a 3.0 mm query, so the diameter
law is contributing almost nothing. **The `D^0.61` exponent is not
implicated in this cell.** (It remains repo-derived and open — CREDITS
says so — but it is not what makes this cell red.)

---

## 5. G-SUB1MM — diagnosis

**Test**
`tests/vendor_lut_sub_1mm.rs:78`,
`sub_1mm_tapered_ball_hardwood_finish_extrapolates_with_scaling`.
Pre-existing red.

```rust
let expected_scale = 0.5 / result.row_diameter_mm;
assert!((result.chipload_diameter_scale - expected_scale).abs() < 1e-6);
```

Measured:

| quantity | value |
|---|---|
| matched row | `amana-tapered-hardwood-parallel-3175-2f` (Ø3.175) |
| `chipload_diameter_ratio_raw` (raw transfer ratio) | **0.157480314960630** |
| test's `expected_scale` = `0.5 / 3.175` | **0.157480314960630** |
| `chipload_diameter_scale` (**applied**) | **0.323823250279165** |
| `(0.5 / 3.175)^0.61` | **0.323823250279165** |
| `CHIPLOAD_DIAMETER_EXPONENT` | **0.61** |

**Diagnosis: the expected value is stale, and by exactly one shipped
change.** The test was written when the exponent was `1.0`, where the
applied scale and the raw ratio coincided. `CHIPLOAD_DIAMETER_EXPONENT`
moved to 0.61 on 2026-08-06 (B-lit §4.1) and this assertion was not
carried. The crate already carries the field that *does* equal the
test's expected value —
`LookupResult::chipload_diameter_ratio_raw`, added in the same wave and
documented as existing "precisely so they cannot be conflated now that
they differ". This test is the conflation the field was added to
prevent.

**Neither the law nor the extrapolation flag is implicated**: the test's
other two assertions (`is_extrapolated`, `row_diameter_mm >= 1.0`) both
pass, and `is_extrapolated` is deliberately read on the *raw* ratio
(`vendor_lookup.rs:98-111`), which the exponent move did not touch.

---

## 6. Checkpoint K — questions, options, recommendations

### (a) Unify the LUT resolver, or keep two with declared purposes?

**Evidence**: §1 (entry points, 141/18 144, all "Suggest unbanded") and
§2 (reroute, 489/3 024 different rows + 378 refusals, ×0.16…×6.17).

| option | what it does | cost |
|---|---|---|
| **a1** unify on the envelope resolver | Suggest also excludes RPM-only rows | Suggest loses the RPM anchor on 141 queries; the anchors exist deliberately, and this pushes against the standing "dial, don't reject" preference |
| **a2** unify on the geometry resolver | the gate accepts RPM-only rows | the gate then has no envelope on those rows → `Unmodeled(NoVendorData)` where it currently judges. Strictly worse |
| **a3** keep two, declare the purposes, and **make the fallback visible** | rename to say what each is for; when Suggest falls back to the formula because its row is RPM-only, emit a typed finding | 141 queries gain a disclosure they do not have; no number moves |
| **a4** — the axis-2 question, **independent of a1–a3** — apply `routed_lookup_family` on the Suggest side too, so both consumers query one family | closes 489 divergences and the 378 refusals in one move | **number-moving**: every Adaptive3d and ProjectCurve recommendation changes. On the reference case Suggest's band max drops ×1/1.2727 = 0.786 |

**Recommendation: (a3) + (a4), in that order, with (a4) sequenced as its
own red-first change.**

Rationale: the two entry points are *not* the defect — the census says
they agree on 4 350 of 4 491 resolved queries and never produce two
different bands. Declaring their purposes (a3) is honest and free. The
divergence with real consequences is the reroute, and the fix there is
not "pick one resolver" but "route the query once". Concretely: hoist
the routing into a single `lut_query_for(operation_kind, tool, …)` that
both `vendor_normalize::to_lookup_query` and `matched_chip_envelope`
call, so the reroute cannot be applied on one side only.

Two things (a4) must decide explicitly, because they are policy not
plumbing:

1. **the ProjectCurve refusal.** If Suggest routes like the gate, then
   on bull-nose / V-bit ProjectCurve Suggest must also refuse — 378
   currently-banded recommendations become "no vendor data". That is
   more honest and less useful. The alternative is to make the gate stop
   refusing, which needs V-bit/bull ProjectCurve rows that the LUT does
   not have.
2. **the forced `Finish` role.** The gate overrides the operation's own
   pass role for ProjectCurve. Suggest adopting that means a roughing
   ProjectCurve is recommended against a finish row.

### (b) The boundary contract

**Evidence**: §3.5 — every gate is already `Within` at exact equality,
so "make the comparisons agree" is a no-op. §3.2 — the defect is
reconstruction, ±1 ulp, 6–8 % of the (rpm, flutes) grid.

| option | |
|---|---|
| **b1** state the contract on `ChipBounds` as **inclusive-with-epsilon at both ends**, with a relative epsilon (`≈ 8 ulp`, i.e. `~1.8e-15` relative) applied by the *bounds type*, not by each gate | the ledger's own proposal |
| **b2** compare on the feed, not the advance — have the gate reconstruct the *band* into feed units once (`max × rpm × flutes`) and compare feeds | removes the round trip entirely on the high side; changes what `observed_mm_per_tooth` means on the wire |
| **b3** widen `ToleranceBands::breakage` from 0.0 | blunt: it moves every verdict, not just boundary ones, and it is the optimizer's dial not a correctness dial |

**Recommendation: (b1)**, stated on `ChipBounds` as a method
(`contains(observed)` / `exceeds_high(observed)`) so no gate writes a
bare `>` again, with the epsilon **relative and named** (suggest
`BOUNDARY_EPSILON_REL = 8.0 * f64::EPSILON`; the measured worst-case
reconstruction error in the census is 1 ulp, so 8 is 8× headroom and
still ~4×10⁻¹⁵ — far below any physically meaningful chipload
difference). Reject (b3): it is the wrong dial and it moves non-boundary
verdicts.

**Also apply (b1) to the drill gates**, which today have no dial at all.
They do not currently exhibit the defect (single division, no round
trip), but "has no epsilon because nothing has bitten yet" is not a
contract.

### (c) The floor-clamped case must report *clamped*, not *exceeds*

**Evidence**: §3.1 — the recipe is on the ceiling **by design**, and the
hints already say `ChiploadClampedToFloor { band_capped_from: Some(…) }`.
The verdict surface says `Exceeds(High)`.

| option | |
|---|---|
| **c1** a new verdict arm, `ChiploadVerdict::ClampedToBandCeiling { .. }` | most explicit; every consumer must handle it |
| **c2** keep `Within`, add a structured advisory alongside `burn_advisory` (e.g. `ceiling_advisory`) | mirrors the existing shape exactly; `low_side_is_advisory` already establishes the pattern for demoting a weakly-founded trip |
| **c3** leave the verdict and fix only the printed string | cosmetic; the optimizer and export gates still see `Exceeds` |

**Recommendation: (c2).** The crate already has the pattern
(`burn_advisory` on the `Within` arm, `chipload.rs:910-945`) and it was
introduced for the structurally identical reason: a bound whose
provenance makes a hard trip indefensible. Here the bound is sound but
the *operating point was chosen by the engine's own clamp*, so a hard
trip is the engine failing its own recipe. (c2) also avoids a wire-shape
change on a verdict enum that MCP, GUI and CLI all consume.

**Precondition**: (c) is only correct **with (b1)**. Without the
epsilon, (c2) would demote genuine 5 %-over exceedances on any op whose
recipe happens to sit near the ceiling. The advisory must be gated on
"the observation is within the boundary epsilon of the ceiling **and**
the recipe carries `ChiploadClampedToFloor`", not on proximity alone.

### (d) Account the floor clamp as a binding constraint

**Evidence**: §3.4 — no `BindingConstraint` variant denotes it, and
`ChiploadMin` is 2.000× away from it on the reference case while its
docstring claims to be it.

| option | |
|---|---|
| **d1** add `BindingConstraint::RubbingFloor` and emit it wherever `effective_rubbing_floor` decided a feed | complete; touches the modulator's vocabulary, which is a wire type |
| **d2** carry the clamp on the *recipe* record (`FeedsResult` / `FeedExplanation`) rather than in the modulator's binding map | the clamp is a Suggest-stage fact, and `FeedExplanation` is already the five-stage record built for exactly this |
| **d3** both — the variant for the modulator, the stage flag for Suggest | |

**Recommendation: (d2), plus the free docstring correction to
`ChiploadMin`.**

The clamp does not happen in the modulator. `ModulationSummary` reports
per-move solver outcomes; Suggest's Step-9b clamp is a whole-recipe
decision taken before any move exists. Putting it in the modulator's
distribution would mean reporting a constraint the modulator never
evaluated — the same category error as the `ChiploadMin` docstring.
`FeedExplanation` already carries `CommandedStage` / `LutBandStage` /
`AchievedFeedStage` / `GateObservationStage`; a `clamped_to:
Option<ClampReason>` on the commanded stage says the true thing in the
right place, and every renderer already prints that record.

If the operator wants (d1) as well, it should carry the *distinct*
meaning "the modulator's own floor bound this move", which is the
existing `ChiploadMin` renamed honestly — not a second name for the
Suggest clamp.

### (e) G-LIT-IPE disposition

**Evidence**: §4 — empty intersection between the cell's expected band
(0.013–0.027) and the matched vendor band (0.0297–0.0744); the
anti-test's stated premise (Ipe matching oak) is false at 0.59×.

| option | |
|---|---|
| **e1** re-verify the cell's three sources via `/refresh-lit-matrix` (S-2 already scheduled) and re-baseline the expected band if the literature does not support 0.013–0.027 for Ø3 hardwood | evidence-first; may exonerate the engine outright |
| **e2** keep the band, re-express the anti-test as **relative** — `ipe_fpt / oak_fpt > 0.85` — so it measures the property its name states instead of an absolute bar | fixes a real instrument defect regardless of e1 |
| **e3** treat the shipped Ø3 hardwood rows as the defect and re-derate them | number-moving across the whole sub-Ø3.175 hardwood surface, on no new source |
| **e4** mark the cell `unadvised` (caps the verdict at minor) | hides it |

**Recommendation: (e2) now, (e1) as the resolution.**

(e2) is independently correct and cheap: an anti-pattern named
`ipe_micro_matches_oak_micro_chipload` should compare Ipe to oak. As an
absolute bar it is a *duplicate* of the `feed_per_tooth` band row, which
is already Outside and already reported at moderate — so the cell's
critical verdict is currently produced by a redundant instrument
measuring the wrong thing. Re-expressed relatively it would read
**0.59 → clear**, and the cell would report the honest finding
(fpt +33.4 % over band, doc +20 %, plunge −61 %) at moderate.

Do **not** take (e3) without (e1). Reject (e4) outright — this cell is
detecting a genuine ~2× disagreement and hiding it would be the exact
failure mode the matrix exists to prevent.

**Also flag, separately from the anti-test**: the same cell's
`plunge/feed` is **−61.1 %** below the expected fraction. That is not a
chipload question and is not covered by any option above; it wants its
own ledger row.

### (f) G-SUB1MM disposition

**Evidence**: §5 — the applied scale is `(0.5/3.175)^0.61` and the test
expects `0.5/3.175`; the crate already publishes the latter as
`chipload_diameter_ratio_raw`.

| option | |
|---|---|
| **f1** re-point the assertion at `chipload_diameter_ratio_raw`, and add a second assertion that `chipload_diameter_scale == raw.powf(CHIPLOAD_DIAMETER_EXPONENT)` | exponent-agnostic; preserves the test's stated spirit ("closest match + scaling + extrapolation flag"); survives the next exponent move |
| **f2** hard-code `0.3238` | re-breaks on the next exponent move; asserts a value, not a property |
| **f3** compute the expectation with `CHIPLOAD_DIAMETER_EXPONENT` inline | equivalent to f1's second half but drops the raw-ratio check, which is the half that catches a *conflation* rather than a *value* |

**Recommendation: (f1).** One commit, no number moves, and it restores
the separation the two fields were introduced to enforce. This is a
straightforward stale-expectation fix and needs no ruling beyond
confirming the operator wants the *property* asserted rather than the
value.

### (g) Bundled from A-5i — the CLI `--adaptive-feed-modulation` default

Checkpoint J flipped `SimulationOptions::default().adaptive_feed_modulation`
to `true` and A-5i confirmed **no shipped consumer inherits the
default**: the GUI pins `true`, the CLI `project` subcommand exposes a
clap flag defaulting **`false`**, `cli smoke` pins `false`, and the
optimizer candidate path pins `false`. So GUI and CLI now disagree on
whether a simulated verdict is modulated.

This matters because of §3: **modulation is what parks a feed on the
band ceiling** on the DropCutter fixtures (A-5 measured
`ConstrainedMax` rewriting 100 % of moves and landing the observation
exactly on the band maximum). The CLI's `Within`/`Exceeds` verdicts are
therefore taken at a different operating point than the GUI's, for the
same project.

| option | |
|---|---|
| **g1** flip the clap default to `true`; keep `--no-adaptive-feed-modulation` to opt out | one default, one behaviour; **is a user-facing default change** — CLI verdicts and reported feeds move for every existing invocation |
| **g2** keep `false`, and make the CLI **print the modulation state** on every verdict it reports | no number moves; the divergence stays but stops being silent |
| **g3** remove the flag and always inherit `SimulationOptions::default()` | least surface; loses the ability to reproduce an unmodulated verdict, which A-5's own evidence needed |

**Recommendation: (g1), with (g2)'s disclosure shipped either way.**
A verdict that depends on an invisible flag is the problem; a flag whose
default differs by front-end is the same problem twice. `cli smoke`
should keep pinning `false` explicitly (it is a fingerprint harness and
must not inherit). If the operator prefers not to move CLI numbers now,
**(g2) alone is an acceptable holding position** — but then the
divergence should be ledgered rather than carried.

---

## 7. NOT EXERCISED / NOT FIXED — with blockers

- **NOT EXERCISED: a live GUI surface.** §0 rule 3. Every number here is
  from the core library through committed harnesses. The chipload
  heat-map, the feeds modal's clamp wording, and the `Exceeds` string on
  a floor-clamped op are all owed a screenshot. Blocker: none technical
  — the wave's evidence is numeric and the surfaces do not move until
  A-7. Owner: A-7, which changes what those surfaces print. **Recorded
  as a gap, not discharged.**
- **NOT EXERCISED: a full pipeline reproduction of G-CHIP-ULP.** The
  fixture drives `tool_load::chipload::evaluate` with a synthetic
  one-sample trace. That is deliberate — it isolates the boundary
  arithmetic from every other moving part — but it means the *live*
  wanaka reading (0.012781077012073162) is quoted from the ledger, not
  re-measured. Blocker: it needs a GUI session on a wanaka-scale
  project. Re-open condition: A-7's live validation.
- **NOT EXERCISED: `routed_lookup_family` called directly.** It is
  `pub(crate)`; §2's table **mirrors** its two rules. The function's own
  behaviour is pinned inside the crate (`chipload.rs:1325-1374`, four
  unit tests), so the mirror only has to stay in step with those — but
  it is a mirror and can drift. If (a4) lands, the mirror should be
  deleted in favour of the unified query builder.
- **NOT MEASURED: whether the reroute is *right*.** §2 measures that the
  two sides disagree and by how much. It does **not** adjudicate whether
  Adaptive3d should be judged against a Pocket envelope. The reroute's
  justification is real and documented (G16 §10 sign-off: vendor 2D
  adaptive rows narrow stepover by design; operators want 2.5–3 mm on
  Adaptive3d). (a4) as recommended keeps that judgement and applies it
  to both sides — it does not re-open it.
- **NOT DIAGNOSED: the `plunge/feed −61.1 %` row** on the Ipe cell.
  Named in (e); no owner assigned.
- **NOT FIXED: anything.** A-6 is a research wave. `G-LIT-IPE` and
  `G-SUB1MM` remain red at HEAD, as instructed — both are pre-existing
  and neither is a TD3 regression.
- **NOT RE-RUN: the full core suite.** The wave added two test files and
  changed no `src`. `cargo clippy -p rs_cam_core --all-targets -D
  warnings` and `cargo fmt --check --all` are clean; both new files pass
  in full. The known-red set is unchanged by construction (no source
  edit), and re-running the whole suite would have occupied the Cargo
  slot for no new information.
