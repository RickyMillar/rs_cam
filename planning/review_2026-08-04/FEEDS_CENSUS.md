# Feeds / Suggest / tool-load census — H1 (R1) research deliverable

Date: 2026-08-04
Basis: `planning/review_2026-08-04/TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md` §H1/R1,
§2; `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md` §5.1 (ledger **B3**);
`planning/review_2026-07-29/ORCHESTRATION_LOG.md` CONCERN 1–2 (live 2026-07-30).
Modelled on `planning/review_2026-07-29/TOOL_SCALE_SEMANTICS.md`.
HEAD measured: `14b0f70`.
Status: **research only. This document proposes no code change.** Every numeric
proposal in §8 is a Checkpoint B item, not a fix.

Method: per-concept `rg` sweeps over `crates/*/src` (production only; `#[cfg(test)]`
bodies excluded by brace-matched module range), then per-site reading of the
enclosing function. Where a claim rests on arithmetic rather than a read, the
arithmetic is shown inline with its inputs so it can be checked without a build.
**No Cargo command was run** — the single machine-wide slot was held by W1
(`standing_material_channel_am9`, then `-p rs_cam_core --lib`) plus a foreign
`spec-index` job for the whole of this wave. Everything marked
`NOT RUN — slot unavailable` in §9 is a claim this document deliberately does
not make.

---

## 0. Executive summary

### 0.1 Distinct-implementation counts (the headline)

"Distinct implementation" = an independent expression of the same physical
concept that could drift; a shared helper called from N sites counts as **1**.

| # | Concept | Distinct implementations | Shared helper? | Can they disagree today? |
|---|---|---:|---|---|
| C-1 | **feed per tooth** `feed / (rpm · flutes)` | **7** | one exists (`stamping::chipload_mm_per_tooth`) and only the simulator calls it | no (identical algebra) — but there is no single owner, so 7 sites must be edited together |
| C-2 | **engaged / effective diameter at DOC** | **3** (LUT semantics, chip-thinning semantics, cutter-trait) | partly: C3 collapsed the tapered arm | **yes** — Flat/Ball/Bull differ by definition between #1 and #3 |
| C-3 | **LUT operation-family map** | **2** | `vendor_normalize::op_family_to_lut` exists; `gcode/mod.rs` re-inlines it | not today (identical arms); a new family diverges silently |
| C-4 | **chipload-envelope row selection** | **2** entry points with different filters | `matched_chip_envelope` is canonical for gate/optimizer/viewport; Suggest does not use it | **yes** — Suggest can aim at a different LUT row than the gate judges against |
| C-5 | **DOC-derating of the chipload band** | 1 implementation, **3 `doc_ratio` semantics** | `geometry::derate_chipload_bounds` (S.8) | **yes** — the three denominators are different diameters |
| C-6 | **chipload ← diameter scaling law** | **2** (`^1.0` LUT, `^0.61` formula) | none | **yes**, by construction |
| C-7 | **chipload ← hardness scaling law** | **2** (`^1.0` LUT, `^1.26` formula) | none | **yes**, by construction |
| C-8 | **spindle power** | 1 formula, **2 widths**, **2 ceilings** | `power::predicted_power_kw` | **yes** — Suggest omits `safety_factor`; widths differ ~1.2× |
| C-9 | **tip deflection** | **1** | `predict::tip_deflection_from_engagement` → `feeds::force` | no — the only fully unified criterion |
| C-10 | **steady-state sample population** | **2** | `locality::is_steady_state_for_gate` | **yes** — only chipload applies the 95 %-commanded-feed filter |
| C-11 | **effective feed for a sample** | 1 helper, **2 application shapes** | `tool_load::effective_feed_for_sample` | shape differs (ratio-scale vs re-derive) + one extra short-circuit |
| C-12 | **chipload floor** | **2** (global 0.025 const; LUT band min) | none | **yes** — the constant exceeds the whole derated band on small tools |
| C-13 | **"what the gate will observe"** | **2 structurally different models** | none | **yes** — order-of-magnitude, see §4 |

Production `chipload`-family sites inventoried: **≈120** across four crates
(§2). Of these, **4 are independent derivations of the same nominal number**
(feeds calculator, simulator, narration, GUI modal) and only **1** flows into a
gate verdict.

### 0.2 Three findings dominate

1. **The four B3 numbers reconcile exactly — as four *different quantities*,
   two of which must not be compared.** The gate's "observed 0.000737" is an
   **arc-mean chip thickness at the LUT row's nominal engagement arc, evaluated
   at the kinematically-predicted feed**; the band it is compared against is a
   vendor **feed-per-tooth**. On the live row those differ by
   `1 / mean_chip_factor(0.586 rad) = 12.4×` before the −87 % predicted-feed
   factor is applied. The reconciliation closes to 0.24 % (§4.3). **This is a
   unit mismatch inside one comparison, not four independent bugs.**
2. **Nobody surfaces that the same operation ran at 7.8× the band maximum on
   the axis the vendor actually publishes.** Narration nominal 0.0714 mm/tooth
   vs band max 0.00916 mm/tooth is a *valid* comparison (same unit, same stage)
   and it is the only one of the four pairs that is both valid and alarming. It
   is computed in `narrate.rs:426` and never reaches a gate.
3. **`RUBBING_FLOOR_MM_TOOTH` (0.025, `feeds/mod.rs:684`) is a global constant
   in a system where every other chipload bound scales with diameter.** On the
   live tool the derated band is 0.00458–0.00916 and the floor is **2.7× above
   the band maximum**. `feeds::calculate` therefore clamps *up* to a value the
   post-sim gate would classify `Exceeds(High)` if it measured on the same axis.
   Two shipped policies contradict on a GUI-reachable tool size.

### 0.3 What is *not* broken

- **Deflection is genuinely unified.** Suggest's predictor, the pre-sim
  cutter-axial envelope, the optimizer preflight and the post-sim gate all reach
  `feeds::force::lateral_cutting_force` through
  `predict::tip_deflection_from_engagement`. It is the worked example the rest
  of the stack should be measured against.
- **`effective_feed_for_sample` is honoured by all three gates.** The plan's
  cited precedent holds: `chipload.rs:556`, `power.rs:206`, `deflection.rs:222`.
  The remaining defect is *how* it is applied, not *whether*.
- **C3's tapered-width unification held.** `feeds::effective_diameter`'s tapered
  arm delegates to `engaged_diameter_at_doc` (`feeds/mod.rs:1748-1750`), and the
  cross-shape sweep sentry
  `feeds::tests::engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`
  keeps hint-level and trait-level geometry honest.
- **The F3.3 advisory ruling and the H4 disclosure fix are intact and correct.**
  `low_side_is_advisory` (`verdict.rs:632`) is the entire discriminator, and
  `from_tool_load.rs:119-125` now discloses observed + floor + `row_id()`.

---

## 1. Non-negotiables this document is written under

- **§2 rule 1 (research first).** No production file is touched by this wave.
- **§2 rule 9.** `planning/airrun_2026-06-01/wanaka.toml` is read-only; the live
  values below are quoted from the 2026-07-30 log, not re-derived from the file.
- **§2 rule 10.** One Cargo job machine-wide. This wave ran none. §9 is the
  honest ledger of what that costs.
- **H1 dependency.** *No feed number an operator acts on moves before
  Checkpoint B.* §8 tiers 3 and 4 are proposals only.
- **Standing operator rules preserved as constraints, not hypotheses:** sim
  chipload is the operational arbiter when observed data exists; hardness dials
  values rather than hard-rejecting rows; Suggest's `ChiploadBounds` mirrors the
  gate's piecewise DOC derating from `feeds::geometry`; gates filter transit
  samples through `locality::is_steady_state_for_gate`; B6 severity stays `Info`.

---

## 2. Implementation census

### 2.1 C-1 — feed per tooth (`feed / (rpm · flutes)`), mm/tooth

Seven production derivations of one division. No owner.

| site | stage | consumer | notes |
|---|---|---|---|
| `feeds/mod.rs:1340-1342` `fpt_divisor` / `commanded_fpt` | Suggest, Step 9b | rubbing-floor clamp | the only one that can *write back* a feed |
| `feeds/mod.rs:1404` `kept_fpt` | Suggest, Step 9c (drill) | RPM follow-down | preserves fpt when the drill envelope caps feed |
| `feeds/suggest.rs:1220` | Suggest, axial envelope | envelope check | |
| `feeds/predict.rs:667-668` | Suggest, pre-sim prediction | `ObservedChiploadPrediction::nominal_mm_per_tooth` | feeds the feed-up recalibration |
| `feeds/predict.rs:248-251` `chipload_per_tooth_mm` | Suggest, deflection breakdown | `DeflectionBreakdown` | |
| `dexel_stock/stamping.rs:786-795` `chipload_mm_per_tooth(...)` | simulation | `SimulationCutSample::chipload_mm_per_tooth` | **the only extracted helper**; called from `dexel_stock/simulation.rs:476-480` |
| `narrate.rs:426` | narration | `"nominal chipload {:.4}mm/tooth"` (`:427`) | reads `ToolpathNarrationContext::{feed_rate_mm_min, spindle_rpm, flute_count}` (`narrate.rs:60-61`); **no derates, no arc, no predicted feed** |
| `rs_cam_viz/src/ui/feeds_modal.rs:381-389` `CurrentValues::chipload_mm()` | GUI | comparison row `:551`, rubbing warning `:823` | 4th independent derivation |
| `tool_load/deflection.rs:224-228` `eff_fz` | gate | force model | **effective**-feed variant — legitimately different |

### 2.2 C-2 — engaged / effective diameter at DOC

| implementation | file:symbol | semantics | Flat | Ball @ 0.05 mm | consumers |
|---|---|---|---|---|---|
| **LUT semantics** | `feeds/mod.rs:123` `ToolGeometryHint::engaged_diameter_at_doc` | "which vendor row applies" | `D` | `D` | `feeds::calculate:783` (RPM/SFM, formula chipload, chipload-band derating ratio), `vendor_normalize`, GUI attestation `feeds_modal.rs:857` |
| **LUT semantics (trait)** | `tool/mod.rs:312` `MillingCutter::lookup_diameter_at` | same contract for a real cutter | `D` | `D` | `tool_load/chipload.rs:401,430`; `tool_load/mod.rs:253,265` |
| **chip-thinning semantics** | `feeds/mod.rs:1735` `effective_diameter` | "what actually touches material" | `D` | `0.44 mm` | `feeds::calculate:1168` (RCTF + axial thinning); **published as `FeedsResult::effective_diameter_mm`** |

Kept honest between #1 and #2 by the cross-shape DOC sweep sentry
(`feeds/mod.rs:118`). **#3 is not in that sentry and is not meant to be** — it
answers a different question. The defect is that #3 is what gets *published*
under a name (`effective_diameter_mm`) whose doc comment (`feeds/mod.rs:443-449`)
claims it is the denominator of the chipload-band derating ratio. It is not:
`calculate` derates with #1 (`:890-896`), and the only downstream consumer,
`suggest::recompute_chipload_bounds_for_dpp` (`suggest.rs:1447-1451`), derates
with #3. See F-3 in §6.3.

Shape primitives behind #3: `geometry::ball_effective_diameter:37`,
`geometry::bull_nose_effective_diameter:80`, `geometry::vbit_width_at_depth:282`.
`geometry::tapered_ball_effective_diameter` was retired by C3; the tombstone at
`geometry.rs:53-74` is accurate.

### 2.3 C-3/C-4 — LUT routing and row selection

| concern | Suggest path | gate / optimizer / viewport path |
|---|---|---|
| op-family map | `vendor_normalize::op_family_to_lut:12` (via `to_lookup_query:26`) | `tool_load/mod.rs:222` uses the same helper; **`gcode/mod.rs:474-483` re-inlines it** |
| op rerouting (ProjectCurve, Adaptive3d) | **absent** | `chipload::routed_lookup_family:801` |
| lookup entry point | `find_best_row_for_geometry` (`feeds/mod.rs:852`) | `find_best_chip_envelope_row` (`chipload.rs:97`) |
| lookup diameter | `engaged_diameter_at_doc(commanded DOC)` | `lookup_diameter_at(peak measured steady-state DOC)` |
| Custom material | not refused here — `material_to_lut` fabricates a Janka from `feed_scale_factor × 600` (`vendor_normalize.rs:206-212`) | refused: `matched_chip_envelope:77-79` returns `None` |

**C-4 is a live divergence.** `find_best_row_for_geometry` will happily return
an RPM-only row (no chipload band); `feeds/mod.rs:918` then detects
`chip_load_mm == 0.0` and swaps to formula chipload while still reporting
`vendor_source = Some(observation_id)` (`:936-937`) — a mixed-provenance result.
The gate, using `find_best_chip_envelope_row`, would have skipped that row and
matched a *different* one. Suggest's band and the gate's band can therefore come
from different rows for the same toolpath. No sentry covers this.

### 2.4 C-5 — the three `doc_ratio` denominators

One shared derating helper (`geometry::derate_chipload_bounds:232`,
`geometry::doc_derating_scale:143`), four call sites, three semantics:

| call site | numerator | denominator | policy |
|---|---|---|---|
| `feeds/mod.rs:890-906` | `input.axial_depth_mm.unwrap_or(tool_diameter)` (`:782`) | **C-2 #1** `engaged_diameter_at_doc` | `RequireBoth` |
| `feeds/suggest.rs:1447-1458` | post-mutation `depth_per_pass` | **C-2 #3** `effective_diameter_mm` | `RequireBoth` |
| `tool_load/chipload.rs:430-437` | peak **measured** steady-state `axial_doc_mm` | **C-2 #2** `lookup_diameter_at(peak)` | `AllowHalfBand` |
| `tool_load/mod.rs:265-273` | same as gate | same as gate | `RequireBoth` |

Consequences, in order of severity:
- On a **ball nose at shallow DOC** the Suggest re-derivation divides by the
  contact circle (0.44 mm at 0.05 mm DOC on a Ø6 ball) instead of the nominal
  diameter, producing a `doc_ratio` ~13.6× larger and saturating the derate at
  **0.5×** where the gate applies **1.0×**. Suggest then aims at half the band
  the gate will use. Only fires when the axial-envelope pass mutated DPP
  (`suggest.rs:1492`).
- When `axial_depth_mm` is `None`, `feeds::calculate` derates at
  `ratio = D / effective_d`, i.e. **1.0 for Flat/Ball/Bull** (no derate) and
  **>1 for V-bit/tapered** — a DOC the operation will not run, because the real
  DOC is chosen later at `:1091-1092` from `operation_default_profile`.
- The doc comment on `FeedsResult::effective_diameter_mm` (`feeds/mod.rs:445-447`)
  states the derating denominator *is* that field. It is not. That comment is
  the exact "stale rationale outliving the code" class P11 names.

### 2.5 C-6 / C-7 — two scaling laws each

| axis | LUT path | formula path |
|---|---|---|
| diameter | `vendor_lookup.rs:265-268` `diameter_scale_factor` = `(q/o).clamp(0.1, 10.0)` — **exponent 1.0** | `feeds/mod.rs:835` `k0 · D^p`, `p = 0.61` (`machine.rs:40`) |
| hardness | `vendor_lookup.rs:306-337` `hardness_scale_factor` = `(row_h / query_h).clamp(0.1, 10.0)` — **exponent 1.0**, inverse | `feeds/mod.rs:835` `(1/feed_scale)^q`, `q = 1.26` (`machine.rs:41`) |

The two exponents are the same crate's answer to the same physical question.
On the live B3 case the LUT law gives band scale 0.3816 where the crate's own
empirical exponent would give `0.3816^0.61 = 0.556` — a **1.46× lower band**
than the formula path believes. Neither is cited to a primary source in
`sources.toml`; `dapra_rctf` covers chip *thinning*, not diameter→chipload.

`family_default_janka` (`vendor_lookup.rs:291-304`) **invents** a hardness anchor
for rows that publish none, with no flag distinguishing a synthetic anchor from a
calibrated ratio. This is the mechanism `_litmatrix_ipe_janka_scaling.rs`
deliberately relies on, so it is load-bearing — but it is provenance-silent.

### 2.6 C-8 — power: one formula, two inputs, two ceilings

| | Suggest (`feeds::calculate` Steps 6 / final) | gate (`tool_load::power::evaluate`) |
|---|---|---|
| formula | `power::predicted_power_kw` (`power.rs:68`) | same helper (`power.rs:213`) |
| cross-section | `ToolGeometryHint::mrr_cross_section_mm2(ap, ae)` (`feeds/mod.rs:1239, 1427`), `ae` = **commanded stepover** | `tool.mrr_cross_section_mm2(axial_doc, radial_width)` (`power.rs:212`) with `radial_width = (arc/π)·engagement_radius·2` (`power.rs:194`) |
| feed | final Suggest feed | `effective_feed_for_sample` (`power.rs:206`) |
| available power | `machine.power_at_rpm(rpm)` (`feeds/mod.rs:1233`) — **no safety factor** | `machine.power_at_rpm(...) · machine.safety_factor` (`power.rs:214`) |
| sample population | n/a (single operating point) | all cutting samples ≥ 0.02 radial, minus phantom-transit and configured-entry; **no commanded-feed filter** |

At `ae = 0.3·D` the gate's arc-equivalent slab is `0.369·D` — **1.23× the
commanded `ae`**. Combined with the missing `safety_factor` (0.75–0.80 typical,
`machine.rs`), Suggest's power headroom is optimistic by roughly
`1.23 / 0.78 ≈ 1.58×` relative to the gate for the same physical cut. Suggest
can therefore decline to power-limit a feed the gate will call `Exceeds`.

### 2.7 C-9 — deflection: unified (the reference implementation)

`tool_load/deflection.rs:106-112` → `predict::tip_deflection_from_engagement`
(`predict.rs:417`) → `feeds::force::lateral_cutting_force` (`force.rs:122`),
`force::immersion_angle` (`force.rs:86`), `force::affine_coefficients`
(`force.rs:109`). Suggest reaches the same three through
`predict::predict_peak_deflection_um` (`predict.rs:127`, immersion at `:315`,
force at `:316`); the cutter-axial envelope through
`cutter_constraints.rs:279`; the optimizer preflight through
`optimize/preflight.rs:185`. The GUI viewport mirrors the gate exactly at
`rs_cam_viz/src/app/simulation.rs:532`.

Residual difference (legitimate, but undeclared): the gate feeds
`sample.axial_engagement_mm` (`deflection.rs:109`) while chipload and power feed
`sample.axial_doc_mm` (`chipload.rs:389`, `power.rs:193,212`). Those are two
different fields on `SimulationCutSample` (`simulation_cut.rs:159` legacy vs
`:163` current). No site declares which is intended.

### 2.8 C-10 / C-11 — sample population and effective feed

| gate | air filter | commanded-feed filter | phantom transit | configured entry | effective feed applied as |
|---|---|---|---|---|---|
| chipload | `radial_woc_fraction < 0.02` (`chipload.rs:247`) | **yes**, `≥ 0.95 × commanded` (`chipload.rs:237,251`) | dropped (`:582`) | routed to `entry_spikes` (`:585`) | **ratio scale** on an already-computed chip (`:553-559`), with an `is_empty()` short-circuit |
| power | same (`power.rs:180`) | **no** | dropped (`:222`) | routed to `entry_spike` (`:225`) | re-derived from raw feed (`:206`) |
| deflection | same (`deflection.rs:210`) | **no** | dropped (`:240`) | routed to `entry_spike` (`:243`) | re-derived to `eff_fz` (`:222-228`) |

Two further asymmetries:
- The chipload gate's commanded-feed filter (`chipload.rs:251`) tests
  `s.feed_rate_mm_min` — the **commanded** feed — while the same loop later
  evaluates chip at the **predicted** feed. On the live op the predicted feed is
  −87 % of commanded, so the filter admits samples whose achieved feed is nowhere
  near steady state and then judges them at that achieved feed. The filter and
  the metric disagree about which feed defines "steady state".
- `chipload_envelopes_for_session` (`tool_load/mod.rs:233-245`) selects its
  lookup DOC through `locality::is_steady_state_for_gate` **plus** `is_cutting`,
  but **not** through the 95 % feed filter that `chipload::evaluate` applies
  (`chipload.rs:386-390` filters the already-feed-filtered set). The viewport
  band and the gate band can therefore be derated at different DOCs.

### 2.9 C-12 — the chipload floor

`RUBBING_FLOOR_MM_TOOTH = 0.025` (`feeds/mod.rs:684`), applied at
`feeds/mod.rs:1341-1352`, warned as `FeedsWarning::ChiploadClampedToFloor`
(`:583`, `:1344`), pinned by `_litmatrix_rubbing_floor_clamp.rs:39` against
`shaw_metal_cutting` / `shaw_chipload`.

It is **diameter-independent**. Every other chipload bound in the system scales
with diameter (C-6). On the live B3 tool the derated band is 0.00458–0.00916
mm/tooth and the floor is 0.025 — **2.73× the band maximum**. The clamp
therefore raises feed to a chipload the gate's own envelope calls breakage-side.
`cells.toml` already encodes diameter-dependence in the opposite direction
(`flat_3mm_pocket_softwood` anti-pattern floor **0.020**, not 0.025), so the
matrix and the constant already disagree at Ø3.

### 2.10 C-13 — two models of "what the gate will observe"

| | Suggest's model | the gate's actual mechanism |
|---|---|---|
| site | `predict::arc_fit_ratio_for_op` (`predict.rs:735-807`) + `predict_observed_chipload_mm:633` | `chipload.rs:522-573` |
| shape | `observed = nominal × k(op_type)`, `k ∈ {0.15, 0.25, 0.30, 0.40, 0.50, 0.60, 0.80}` — a per-`OperationType` constant | `observed = nominal × mean_chip_factor(arc_sample) × (lut_factor / mean_chip_factor(arc_sample)) × (predicted/commanded)` = `nominal × mean_chip_factor(lut_arc) × feed_ratio` |
| depends on | operation type only | the **LUT row's `ae` band** and the **machine kinematics**; independent of operation type |
| provenance | `Calibrated` for Adaptive3d/DropCutter, `Default` elsewhere (`ArcFitRatioSource`, `predict.rs:835`) | none declared |
| consumer | `recalibrate_feed_for_chipload` (`suggest.rs:2059`) — **writes feed** | the verdict |

These are not two calibrations of one model; they are two different models. The
gate's value does not depend on operation type at all once the LUT row is fixed,
which is precisely what Suggest's table keys on. On the live scallop op the
table predicts `0.15 × 0.0714 = 0.0107`; the gate measured `0.000737` — a
**14.5× miss** (§4.3). `recalibrate_feed_for_chipload` only fires when
`source == Calibrated` (`suggest.rs:2073-2081`), i.e. only for Adaptive3d and
DropCutter, which is why this has not produced a runaway feed elsewhere.

---

## 3. Directed data-flow map

```text
 catalog / tool / material / machine
 ├─ OperationSpec{feeds_family, feeds_pass_role}      compute/catalog.rs
 ├─ ToolConfig → build_cutter → ToolDefinition        compute/cutter.rs
 ├─ Material{kc, feed_scale_factor, janka, drill env} material.rs
 └─ MachineProfile{rpm_range, power, safety_factor,
                   max_feed, chip_load{k0,p,q}}       machine.rs
        │
        ▼  vendor_normalize::to_lookup_query:26   (op_family_to_lut:12,
        │                                          material_to_lut:120)
 ┌──────┴──────────────────────────────────────────────────────────────┐
 │ VENDOR LUT                                                          │
 │  VendorLut::embedded  vendor_lut.rs:385  (silently drops bad files) │
 │  ── Suggest branch ──────────  ── gate/optimizer/viewport branch ── │
 │  find_best_row_for_geometry     find_best_chip_envelope_row         │
 │        vendor_lookup.rs:181           vendor_lookup.rs:206          │
 │        (any row, incl. RPM-only)      (chipload-bearing rows only)  │
 │              │                              │                       │
 │  passes_must_match:459  →  score_observation:500  →  build_result:339│
 │              └── scales: diameter:265, hardness:306 (both ^1.0)      │
 └──────┬──────────────────────────────────────┬───────────────────────┘
        │ LookupResult                          │ LookupResult
        ▼                                       ▼
 feeds::calculate  feeds/mod.rs:763       tool_load::chipload::matched_chip_envelope:69
  1 rpm    ← SFM(effective_d #1) → clamp_rpm → vendor rpm_nominal
             → drill tier:702 / milling tier:748 / MaxSpeed:979
  2 chip   ← LUT chip_load_mm, else k0·D^0.61·(1/h)^1.26  :835
  2' band  ← derate_chipload_bounds(RequireBoth, ratio=DOC/eff_d #1) :897
  3 ap,ae  ← operation_default_profile:1493 / user override / scallop :1102
  5 feed   ← rpm·chip·flutes·thinning(RCTF·axial, eff_d #3)·depth_tier :1190
  5b       ×= ld_overhang :1210, workholding :1219
  6 power  ← predicted_power_kw vs power_at_rpm  (NO safety_factor) :1233-1251
  7 clamp  ← cutting_feed_ceiling_mm_min :1256
  9 safety ×= machine.safety_factor :1280
  9b floor ← RUBBING_FLOOR_MM_TOOTH 0.025 :1341   ◄── C-12
  9c drill ← material plunge envelope, RPM follow-down :1376
        │ FeedsResult{rpm, chip_load_mm, feed_rate_mm_min, chipload_bounds,
        │             matched_lut_row, effective_diameter_mm #3, derates}
        ▼
 suggest::apply_feeds_result_to_op:801 ─► enforce_invariants:1465
        ├ pick_axial_envelope:1328 ──► (if DPP mutated)
        │        recompute_chipload_bounds_for_dpp:1433  ◄── C-5 divergence
        ├ clamp_plunge_to_feed / clamp_stepover / rigidity / cutting_length
        ├ backoff_dpp_for_deflection:1722  ──► predict_peak_deflection_um
        └ recalibrate_feed_for_chipload:2059 ◄── C-13
                 └ predict_observed_chipload_mm:633 (arc_fit_ratio table)
        │
        ▼  PERSISTED PARAMS  OperationConfig{feed_rate, plunge_rate, spindle_rpm,
        │                                    depth_per_pass, stepover}
        │  provenance stamped: feeds/provenance.rs:288 FeedsResult::provenance
        ▼
 GENERATED IR   toolpath moves carry feed_rate_mm_min + spindle_rpm
        ├─ feed_modulation.rs:366 (per-move feed ← chipload_band from
        │      chipload_envelopes_for_session, session/compute.rs:2461)
        └─ machine_kinematics → PredictedFeedMap  (F-034/F-035)
        │
        ▼  SIMULATION  dexel_stock/simulation.rs:476-497
        │   chipload_mm_per_tooth (stamping.rs:786) → sample.chipload_mm_per_tooth
        │   effective_chip_thickness_mm:581 = chip_geometry(...).mean_chip_thickness_mm
        │        → sample.effective_chip_thickness_mm      ◄── ARC-MEAN CHIP (mm)
        │   Engagement{ mean_chip_thickness_mm := nominal fpt   ◄── mislabelled
        │               peak_chip_thickness_mm := arc-mean chip } ◄── mislabelled
        ▼
 GATES  gcode::project_load_report:438 → tool_load::evaluate_toolpath:430
        ├ chipload::evaluate:297   steady 95 % filter → arc-normalize → median/peak
        ├ power::evaluate:73       arc-slab width → predicted_power_kw
        ├ deflection::evaluate:116 axial_engagement + eff_fz → feeds::force
        └ drill_gates::evaluate    (drill only)
        │
        ▼  ToolpathLoadVerdict
        ├─ diagnostics/adapters/from_tool_load.rs:119-204 (messages + LutCitation)
        ├─ narrate.rs  (independent nominal at :426 — never reads the verdict)
        ├─ optimize/context.rs:123 (re-uses matched_chip_envelope — no 2nd model)
        │     └ retarget/chipload.rs:100-109  multiplier = target / gate-observed
        ├─ rs_cam_viz sim_timeline.rs:409 (plots raw effective_chip, NOT normalized)
        └─ CLI smoke.rs:588 / MCP app/mcp.rs:4584 (raw per-sample peak)
```

**Four independent nominal-chipload derivations** are visible on this map
(`feeds/mod.rs:1342`, `stamping.rs:786`, `narrate.rs:426`,
`feeds_modal.rs:381`). Only the simulator's flows into a verdict.

---

## 4. B3 reconciliation

### 4.1 The four values, as recorded

From `planning/review_2026-07-29/ORCHESTRATION_LOG.md:1284-1290` and
`SUPERSEDED_CONCLUSIONS.md:566-572`, one operation, live session 2026-07-30:

| # | value | mm/tooth |
|---|---|---:|
| N1 | narration nominal | 0.0714 |
| N2 | `feeds.chipload_clamped_to_floor` pre → post | 0.0044 → 0.0250 |
| N3 | gate observed | 0.000737 |
| N4 | gate band | 0.00458 – 0.00916 |

Matched LUT row (recoverable from N4, see §4.2):
`amana-tapered-hardwood-scallop-3175-2f`
(`crates/rs_cam_core/data/vendor_lut/observations/amana_3d_profiling.json`) —
`chipload_min 0.012`, `chipload_max 0.024`, `diameter_mm 3.175`,
`ae_min_mm 0.08`, `ae_max_mm 0.45`, `hardness_value 1450 janka`,
`pass_role semi_finish`.

### 4.2 Definitions, units and comparison validity

| # | what it actually is | producing symbol | unit | stage | population |
|---|---|---|---|---|---|
| **N1** | commanded **linear advance per tooth**, `feed / (rpm · flutes)` | `narrate.rs:426` | mm of *advance* per tooth | persisted params (pre-IR) | the whole toolpath, one scalar |
| **N2 pre** | Suggest's *would-be* commanded advance per tooth after every derate, at recompute time | `feeds/mod.rs:1342` | mm advance/tooth | Suggest projection | one operating point |
| **N2 post** | the global rubbing constant it was clamped **up** to | `feeds/mod.rs:684` | mm advance/tooth | Suggest policy | — |
| **N3** | **arc-mean chip thickness**, renormalised to the LUT row's nominal engagement arc, evaluated at the **kinematically predicted** feed, taken as the **median** over steady-state non-entry non-phantom samples | `chipload.rs:563-573` → `:620-626` | mm of *chip* | post-sim observation | median of a filtered sample set |
| **N4** | vendor-published chipload band × diameter scale × hardness scale × DOC derate | `vendor_lookup.rs:352-354` → `geometry.rs:251-253` | mm advance/tooth (vendor convention) | LUT | one row |

| pair | same unit? | same stage? | comparison valid? | verdict |
|---|---|---|---|---|
| **N1 vs N4** | ✅ both advance/tooth | ✅ commanded vs authored | **VALID** | the op runs at **7.8× the band max**. Nothing surfaces it. |
| **N2 vs N4** | ✅ | ✅ | **VALID** | the floor (0.0250) is **2.73× the band max** (0.00916). Two policies contradict. |
| **N2pre vs N1** | ✅ | ✅ | **VALID** | 0.0044 vs 0.0714 = **16×**. Suggest's recompute and the persisted value are far apart — expected if the op was hand-tuned or optimizer-written, but nothing labels which. |
| **N3 vs N4** | ❌ chip vs advance | ✅ | **INVALID as printed** | biased low by `mean_chip_factor(lut_arc)`; on this row **12.4×** |
| **N3 vs N1** | ❌ | ❌ commanded vs achieved | **INVALID as printed** | differs by the arc factor **and** the −87 % predicted-feed factor; neither is disclosed |
| **N1 vs N2pre** vs **N3** | — | — | — | three of the four are the *same* physical quantity at different stages; one is a different quantity |

### 4.3 Arithmetic reconciliation of N3

All inputs are file-resident; no build required.

```
LUT nominal arc          chipload.rs:127 lut_nominal_arc_rad
  ae_mid = (0.08 + 0.45)/2                       = 0.2650 mm
  ratio  = 1 − 2·ae_mid / row_diameter
         = 1 − 2·0.2650 / 3.175                  = 0.83307
  arc    = acos(0.83307)                         = 0.58565 rad

LUT arc factor           chipload.rs:109 mean_chip_factor
  h_max  = sin(0.58565)                          = 0.55271
  f_lut  = (2·0.55271 / 0.58565)·(1 − cos(0.29283))
         = 1.88752 · 0.042565                    = 0.080335

Predicted-feed factor    modulation_summary, ORCHESTRATION_LOG.md:1300
  median_feed_delta_pct  = −87.18 %              → 0.12820

Predicted N3
  N1 · f_lut · feed_factor
  = 0.0714 · 0.080335 · 0.12820                  = 0.0007353 mm

Recorded N3                                      = 0.0007371 mm
Residual                                         = +0.24 %
```

The 0.24 % residual is inside the rounding of the quoted N1 (4 d.p.) and the
median-vs-mean difference between the two statistics. **N3 is fully explained.**

Note what the algebra shows: after D9's renormalisation the sample's own
engagement arc **cancels out** —
`cl_norm = fz · f(arc_sample) · f_lut / f(arc_sample) = fz · f_lut`. The gate's
"observed chipload" is, by construction, the commanded feed-per-tooth times a
constant that depends only on the matched LUT row's `ae` band, times the
achieved/commanded feed ratio. It carries **no information about the sample's
actual engagement**, which is the thing the metric is named for.

### 4.4 Verdict: four concepts, or four bugs?

**Neither, cleanly. Three of the four are the same concept at three stages; the
fourth is a different physical quantity wearing the same label. Two genuine
defects and two undisclosed-but-legitimate stage differences.**

| item | classification | evidence |
|---|---|---|
| N1 vs N3 differ by the achieved/commanded feed ratio | **legitimate stage difference, undisclosed** | `effective_feed_for_sample:63` is the intended design (F-035). Nothing in the diagnostic or narration says the observed value is at a predicted feed −87 % below commanded. |
| N3 is an arc-mean chip, N4 is an advance-per-tooth | **DEFECT — unit mismatch inside one comparison** | `chipload.rs:598` `geometry.mean_chip_thickness_mm` vs `vendor_lut.rs:199` `chipload_min_mm_tooth`. The module doc (`chipload.rs:10-17`) asserts the vendor number is an arc-average; no source in `sources.toml` supports that reading, and `_litmatrix_*` pins `RUBBING_FLOOR_MM_TOOTH` against `shaw_chipload` on the **advance** convention. **Calibration decision — Checkpoint B.** |
| N2's floor is diameter-independent while N4 scales with diameter | **DEFECT — policy contradiction** | `feeds/mod.rs:684` const vs `vendor_lookup.rs:268` linear scale; `cells.toml:1161` already uses 0.020 at Ø3 |
| N1 at 7.8× N4 is never gated or surfaced | **DEFECT — missing report** | `narrate.rs:426` computes it; no consumer compares it to `ChiploadBounds` |
| Suggest's `arc_fit_ratio` model vs the gate's arc-normalisation | **DEFECT — two models of one quantity**, 14.5× apart on this op | `predict.rs:735` vs `chipload.rs:522-573`; mitigated only because the feed-up loop is gated on `Calibrated` (`suggest.rs:2073`) |
| the gate's `Within` on N3 < N4 | **NOT a defect** — F3.3, ruled, disclosed by H4 | `verdict.rs:632`, `from_tool_load.rs:119-125` |

Two of the "disagreements" the operator saw are therefore artefacts of a single
root cause: **the gate reports a value in a unit the band is not published in,
and does not disclose the two multipliers between its number and the commanded
one.** A report-only fix (§8 tier 1) closes the *contradiction* without moving
any number. Whether the comparison itself should change unit is Checkpoint B.

---

## 5. LUT state-machine table

The lookup is **not** a typed state machine: `vendor_lookup` returns
`Option<LookupResult>` with a `bool` and two `f64` scales. Typing happens one
layer up in `tool_load::verdict::ChipBoundsSource`.

| state | representation | set at | flag carried | operator-visible? |
|---|---|---|---|---|
| **no row (no match)** | `None` | `vendor_lookup.rs:420` | — | Suggest → `ChiploadSource::FormulaFallback` (`feeds/mod.rs:943`); gate → `Unmodeled(NoVendorData)` (`chipload.rs:413`) |
| **no row (angle-gated)** | `None` | `vendor_lookup.rs:144,161` | — | as above; V-bit only, tolerance ±20° (`:105`) |
| **no chipload-bearing row** | `None` | `vendor_lookup.rs:206-213` | — | gate path only — **Suggest does not use this entry point** (C-4) |
| **matched** | scales `1.0`, `is_extrapolated=false` | `vendor_lookup.rs:349,372,373` | `observation_id`, `score` | `ChipBoundsSource::VendorLut` → `row_id "vendor_lut"`, `Confidence::Validated` |
| **scaled** | scales ≠ 1.0, within ±40 % combined | `vendor_lookup.rs:346-354` | `chipload_diameter_scale`, `chipload_hardness_scale` | still `VendorLut` / `Validated` — **the scaling is invisible in the verdict** |
| **clamped** | scale saturated at 0.1 or 10.0 | `vendor_lookup.rs:268, 319, 331` | **none** | invisible; only implied by `is_extrapolated` |
| **extrapolated** | `is_extrapolated = true` when `\|ln(d_scale · h_scale)\| > ln 1.4` | `vendor_lookup.rs:349`, const `:263` | direction **not** carried (`.abs()`) | `ChipBoundsSource::VendorLutExtrapolated`, `row_id "vendor_lut_extrapolated"`, `Confidence::Approximate(detail)` (`chipload.rs:477-487`), low side → advisory |
| **point preset** | raw `min >= max` | `chipload.rs:455-460` | — | `VendorLutPointPreset`, low side → advisory |
| **no `ae` calibration** | `ae_min` and `ae_max` both `None` | `chipload.rs:463-464` | — | `VendorLutMissingAe`, low side → advisory; **also disables D9 normalisation** (`:522-523`) |
| **rejected (hard filter)** | row skipped | `vendor_lookup.rs:459-498` | — | contributes to "no row" |
| **unusable bounds** | max absent/non-finite/≤0, or present-but-invalid min | `geometry.rs:238-249` → `chipload.rs:443` | — | `Unmodeled(NoVendorData)` |
| **advisory (low side)** | `Within { burn_advisory: Some(..) }` | `chipload.rs:698-708` | metric + bounds + source | `Severity::Info` (`from_tool_load.rs:135`), `Confidence::Approximate` (`:228-236`), message discloses observed + floor + row_id (`:119-125`) |
| **hard trip (high side)** | `Exceeds { side: High }` | `chipload.rs:671-681` | — | `Severity::Caution` (`from_tool_load.rs:176`); **never** advisory-downgraded (`verdict.rs:629-631`) |
| **hard trip (low side)** | `Exceeds { side: Low }` | `chipload.rs:717-721` | — | only when `source == VendorLut` |

**Hardness dials, never rejects — confirmed.** `passes_must_match`
(`vendor_lookup.rs:459-498`) has no hardness clause. Hardness enters only as
(a) the value dial `hardness_scale_factor:306` and (b) a `≤80`-point score nudge
`:526-537`. The operator rule holds in code.

**Filter/score split**, for the record: hard filters are operation family
(exact), material *category*, tool-family compatibility set, diameter ratio
sanity band `0.1..=10.0`, V-bit angle ±20°. Pass role, flute count, hardness,
material *family*, tool subfamily are **score-only** — a `semi_finish` row can
and did win a `finish` query (`−25` points, `:545-549`), which is what happened
on the live B3 op.

**Row-scaling asymmetry (defect candidate):** `build_result` scales
`chip_load{,_min,_max}` by `total_scale` (`vendor_lookup.rs:352-354`) but passes
`rpm_*`, `ap_*_mm`, `ae_*_mm` through **unscaled** (`:355-363`). A row scaled
0.38× on diameter still hands out the original row's absolute mm `ae` band — and
that unscaled `ae` band is exactly what the gate's D9 normalisation reads
(`chipload.rs:128-135`). §8 tier 2.

---

## 6. Parity matrix

Rows are the physical inputs; columns are the five consumers. **≡** = same
value from the same code; **≈** = same concept, different code or input;
**✗** = genuinely different; **—** = not consumed.

### 6.1 Matrix

| input | Suggest (`feeds::calculate` + `suggest::*`) | chipload gate | power gate | deflection gate | optimizer |
|---|---|---|---|---|---|
| LUT row | `find_best_row_for_geometry` | `find_best_chip_envelope_row` | — | — | ≡ gate (`optimize/context.rs:128`) |
| op-family routing | no ProjectCurve/Adaptive3d reroute | `routed_lookup_family:801` | — | — | ≡ gate |
| lookup diameter | `engaged_diameter_at_doc(commanded DOC)` | `lookup_diameter_at(peak measured DOC)` | — | — | `diameter_for_lut_lookup(commanded DOC)` |
| chipload band | ≈ (derated at C-2 #1 or #3) | ≈ (derated at C-2 #2) | — | — | ≡ gate |
| DOC | commanded `depth_per_pass` | peak measured `axial_doc_mm`, steady-state only | per-sample `axial_doc_mm` | per-sample **`axial_engagement_mm`** ✗ | commanded |
| radial engagement | commanded `stepover` | arc, cancelled by D9 | `(arc/π)·engagement_radius·2` ✗ | arc as immersion angle ψ | commanded |
| RPM | Suggest's own tiering + vendor + MaxSpeed | `sample.spindle_rpm` (IR) | `sample.spindle_rpm` | `sample.spindle_rpm` | search axis |
| flute count | `input.flute_count` | `s.flute_count` implicitly via chip geometry | `s.flute_count` implicitly | `s.flute_count` ✓ explicit (`deflection.rs:223`) | tool |
| feed | commanded (its own output) | **effective**, applied as ratio | **effective**, re-derived | **effective**, re-derived | candidate feed → gate |
| spindle power ceiling | `power_at_rpm(rpm)` ✗ | — | `power_at_rpm(rpm) · safety_factor` | — | ≡ gate |
| cross-section | `hint.mrr_cross_section_mm2(ap, ae_commanded)` | — | `tool.mrr_cross_section_mm2(doc, arc_slab)` ✗ | — | ≡ gate |
| Kc / force model | `feeds::force` via `predict` ≡ | — | `kc_n_per_mm2` + `GRAIN_ANISOTROPY_FACTOR` ≡ | `feeds::force` via `predict` ≡ | ≡ |
| deflection model | `predict::predict_peak_deflection_um` ≡ | — | — | `predict::tip_deflection_from_engagement` ≡ | ≡ (`preflight.rs:185`) |
| machine-feed cap | `cutting_feed_ceiling_mm_min()` | — | — | — | `machine.max_feed_mm_min` (`bounds.rs:218`) ✗ |
| Custom material | fabricates a Janka (`vendor_normalize.rs:206`) ✗ | refuses (`chipload.rs:77`) | refuses (`power.rs:127`) | refuses | refuses |
| sample population | n/a | air + **95 % commanded feed** + steady-state | air + steady-state | air + steady-state | ≡ chipload (preflight) |
| DOC derating | `RequireBoth`, 2 different denominators | `AllowHalfBand` | — | — | ≡ gate |

### 6.2 Divergences ranked by product risk

| id | divergence | mechanism | risk |
|---|---|---|---|
| **P-1** | gate's observed unit ≠ band unit | C-13 / §4 | **HIGH** — every chipload verdict on an `ae`-bearing row is biased low by `1/f_lut` (4.3× on a typical roughing row, 12.4× on the live finish row) |
| **P-2** | Suggest's power ceiling omits `safety_factor` | `feeds/mod.rs:1233` vs `power.rs:214` | **HIGH** — Suggest can ship a feed the gate calls `Exceeds`; ~1.28× on its own, ~1.58× with the width difference |
| **P-3** | Suggest and gate can match different LUT rows | C-4 | **HIGH** — the band Suggest aims at is not the band the verdict cites |
| **P-4** | rubbing floor vs LUT band on small tools | C-12 | **HIGH** — clamps *up* past the band max on Ø≲2 mm tools |
| **P-5** | `recompute_chipload_bounds_for_dpp` divides by the chip-thinning diameter | C-5 | **MEDIUM** — up to 2× band error on ball tools at shallow DOC, only after a DPP mutation |
| **P-6** | power's radial width ≠ commanded `ae` | C-8 | **MEDIUM** — ~1.23× at 30 % radial, direction is conservative on the gate side |
| **P-7** | deflection reads `axial_engagement_mm`, others read `axial_doc_mm` | C-9 | **MEDIUM** — undeclared; if the two fields ever diverge, three gates split silently |
| **P-8** | chipload's steady-state filter tests commanded feed, metric uses predicted | C-10 | **MEDIUM** — on a −87 % kinematically-throttled path the filter is measuring the wrong feed |
| **P-9** | power/deflection have no commanded-feed filter | C-10 | **MEDIUM** — their peaks can come from ramp/lead-in samples the chipload gate excludes |
| **P-10** | narration nominal never compared to the band | §4.2 | **MEDIUM** — the only valid alarming comparison is computed and discarded |
| **P-11** | `Engagement::{mean,peak}_chip_thickness_mm` are mislabelled | `simulation_cut.rs:88-96` vs `dexel_stock/simulation.rs:496-497` | **MEDIUM** — `mean_` holds the nominal fpt; `peak_` holds the arc-**mean**; both doc comments assert the opposite. `per_kinematics` summaries and MCP report the mislabelled fields |
| **P-12** | `gcode/mod.rs:474-483` re-inlines `op_family_to_lut` | C-3 | LOW today |
| **P-13** | `chipload_envelopes_for_session` DOC population ≠ gate's | C-10 | LOW — viewport colour vs export verdict |
| **P-14** | `enumerate_matching_rows` bypasses the V-bit angle gate | `vendor_lookup.rs:231-247` | LOW — Explain-UI sibling rows only |
| **P-15** | LUT `ae_*_mm` / `ap_*_mm` unscaled while chipload is scaled | `vendor_lookup.rs:355-363` | LOW–MEDIUM — feeds the D9 arc directly |
| **P-16** | optimizer feed ceiling is `max_feed_mm_min`, Suggest's is `cutting_feed_ceiling_mm_min()` | `bounds.rs:218` vs `feeds/mod.rs:1256` | LOW–MEDIUM — optimizer may propose above the cutting ceiling |
| **P-17** | `family_default_janka` fabricates an anchor with no provenance flag | `vendor_lookup.rs:291-334` | LOW — load-bearing for `_litmatrix_ipe_janka_scaling` |
| **P-18** | `VendorLut::embedded` silently drops unparsable files | `vendor_lut.rs:385-393` | LOW — test-guarded only |

### 6.3 Named findings

- **F-1 (P-1).** `chipload::evaluate` compares
  `chip_geometry(...).mean_chip_thickness_mm` (mm of chip) against
  `chipload_min/max_mm_tooth` (mm of advance). After D9 the sample arc cancels
  and the ratio is exactly `mean_chip_factor(lut_arc)`.
- **F-2 (P-2).** `feeds/mod.rs:1233` `let available_power = machine.power_at_rpm(rpm);`
  — no `· machine.safety_factor`, unlike `power.rs:214`.
- **F-3 (C-5/P-5).** `FeedsResult::effective_diameter_mm`'s doc comment
  (`feeds/mod.rs:445-447`) states the derating denominator; the code uses a
  different diameter (`:783` vs `:1168`, same identifier shadowed).
- **F-4 (P-11).** `Engagement::mean_chip_thickness_mm` is assigned the nominal
  fpt and `peak_chip_thickness_mm` the arc-mean chip
  (`dexel_stock/simulation.rs:496-497`); the field docs at
  `simulation_cut.rs:88-96` assert the reverse, and `:357-362` propagates the
  mislabelled pair into `SimulationCutSummary`.
- **F-5 (C-13).** `arc_fit_ratio_for_op` predicts a quantity the gate does not
  compute. Its `Calibrated` rows (Adaptive3d 0.25, DropCutter 0.15) were fixed
  on 2026-06-03 against a gate that already carried D9 (landed `693bbf5`,
  2026-05-10), so the table is calibration-against-the-artefact, not a stale
  pre-D9 relic — but it is keyed on the wrong variable.
- **F-6 (P-3).** `feeds/mod.rs:852` uses `find_best_row_for_geometry`;
  `chipload.rs:97` uses `find_best_chip_envelope_row`. Both are correct for
  their own purpose; nothing asserts they agree.

---

## 7. Source inventory — literature-matrix rows touched by feed/RPM work

Seven `_litmatrix_*` sentries exist; six are feeds rows. Infrastructure:
`tests/literature_matrix.rs:23-48`, registry
`tests/literature_matrix/sources.toml` (32 tables, key = source id),
cells `tests/literature_matrix/cells.toml`, freshness engine
`tests/literature_matrix/freshness.rs` (`FRESH_MONTHS:31`, `STALE_MONTHS:32`,
`classify:118`, `decay_fail_enabled:358`).

| sentry | axis | pinned threshold | production symbol | sources backing the cell |
|---|---|---|---|---|
| `_litmatrix_drill_rpm_ceiling.rs:33` | drill RPM | ceiling 14 000 | `drill_rpm_envelope_for_diameter` (`feeds/mod.rs:702`), re-clamps `:815`, `:1081`, `:1406` | `onsrud_drill`, `vectric_drill_default`, `amana_spektra` |
| `_litmatrix_drill_rpm_diameter_tier.rs:12-14` | drill RPM | (8000,14000)/(6000,10000)/(4000,8000) | same | `onsrud_drill`, `vectric_drill_default`, `fpl_wood_handbook` |
| `_litmatrix_milling_rpm_diameter_tier.rs:24-28` | milling RPM | 22/20/18/16/14 k by Ø | `milling_rpm_ceiling_for_diameter` (`feeds/mod.rs:748`), `SPINDLE_CEILING_HEADROOM:348`, `MAX_SPINDLE_SPEEDUP:343` | `onsrud_hwood` (series 70/85), `amana_spektra`, `gwizard_hwood` |
| `_litmatrix_rpm_only_lut_chipload.rs:38` | chipload / feed | chipload > 0 **and** ≥ 0.025 | LUT-zero fallback (`feeds/mod.rs:918`), `RUBBING_FLOOR_MM_TOOTH:684` | `shaw_metal_cutting`, `shaw_chipload` |
| `_litmatrix_rubbing_floor_clamp.rs:39` | chipload | floor 0.025 + warn | `feeds/mod.rs:1341-1352`, `FeedsWarning::ChiploadClampedToFloor:583` | `shaw_metal_cutting`, `shaw_chipload`, `fpl_wood_handbook` (Janka) |
| `_litmatrix_ipe_janka_scaling.rs` | chipload (hardness) | relative only: `ipe_fpt < oak_fpt`; anchor 1290 | `vendor_lookup::hardness_scale_factor:306`, `family_default_janka:291` | `fpl_wood_handbook` |
| `_litmatrix_scallop_refuses_flat.rs` | *geometry, not feeds* | typed refusal | `validate_tool_for_operation` (`feeds/mod.rs:656`) | `shaw_metal_cutting` |

**Freshness state at 2026-08-04:** the matrix clock defaults to `2026-06-03`
(`freshness.rs:30`), at which every source reads *fresh*. Against the real date
all 32 sources are **14–15 months old ⇒ `warn`**, none yet `stale` (flips
2026-12-03). Stale is warn-only unless `LIT_MATRIX_DECAY_FAIL=1`
(`freshness.rs:358`, `runner.rs:73`). **NOT RUN — slot unavailable**: the
freshness report and citation audit were not executed this wave.

**Gaps the census must record, not fix:**

1. `audit_citations` only audits `expected.*` bands (`freshness.rs:293-327`);
   **invariant and anti-pattern `sources` are present in `cells.toml` but never
   deserialized, so they are unaudited** (self-documented at `:328-335`). Most
   drill and chipload thresholds live on *invariants*. Their citations are
   currently unchecked by the gate that exists to check them.
2. **No primary source in `sources.toml` justifies the diameter→chipload
   exponent** (C-6), on either the LUT law (`^1.0`) or the formula law
   (`^0.61`). `dapra_rctf` covers radial chip thinning, a different quantity.
3. **No source justifies reading the vendor chipload column as an arc-mean chip
   thickness** (F-1). `shaw_metal_cutting` is cited for the rubbing floor on the
   advance convention.
4. `cells.toml:3737-3738` pins `flat_12mm_drill_oak_big` rpm **3000–6000** and
   floor **2000**, while the sentry (`_litmatrix_drill_rpm_diameter_tier.rs:73-75`)
   and production (`feeds/mod.rs:708`) both use **4000–8000**. Sentry and impl
   agree; the cell band does not. Same cell declares oak Janka 1360 while the
   Ipe sentry anchors oak at 1290.
5. `CREDITS.md` has **no lineage entry naming `onsrud_drill` or
   `vectric_drill_default`** — the two sources behind every drill RPM/peck
   threshold. Drill lineage there is limited to a CNC Cookbook benchmark link
   (`CREDITS.md:436`) and the fiberglass placeholder constants (`:308-312`).
6. Six sources carry `citation_url = "(pending)"` (`sources.toml:251, 261, 301,
   311, 321, 331`); nothing validates that field.

Any change under §8 tiers 3–4 that moves a chipload, RPM or drill threshold
**must** go through `/refresh-lit-matrix` for the rows above, per plan §H1
acceptance gate 6 — not a silent re-pin.

---

## 8. Proposed fix sequence

Four tiers, strictly ordered. **Tier 1 may proceed under PR-7 (report-only).
Tiers 2–4 are Checkpoint B material.** Nothing here is a decision.

### Tier 1 — report-only (no number an operator acts on moves)

| id | change | files | red-first evidence |
|---|---|---|---|
| T1.1 | Introduce a stage/provenance-bearing **feed explanation record**: `{commanded_fpt, suggest_target_fpt, lut_band(row_id, scales, derate), gate_observed(statistic, unit, arc_factor, feed_factor), predicted_feed_ratio}` — one struct, labels only, chooses no winner | new `feeds/explanation.rs` (or extend `feeds/explain.rs`), consumed by `from_tool_load.rs`, `narrate.rs` | fixture asserting all five stages are populated and distinctly named |
| T1.2 | Rename the gate's reported quantity in **wording only**: `"Chipload within band (X mm/tooth)"` → an explicit label naming the statistic and unit, plus the two multipliers | `diagnostics/adapters/from_tool_load.rs:119-204` | assert no bare `chipload` label remains in the touched path (plan §H1 gate 1) |
| T1.3 | Fix **F-4**: swap or rename `Engagement::{mean,peak}_chip_thickness_mm` to match what is assigned, and correct both doc comments | `simulation_cut.rs:88-96, 357-362`; `dexel_stock/simulation.rs:496-497` | a sample with `arc = π/2` must show `peak > mean`; today it does not |
| T1.4 | Fix **F-3**: correct the `effective_diameter_mm` doc comment to name the diameter actually used, and document the shadowing in `calculate` | `feeds/mod.rs:443-449, 783, 1168` | docs-only; paired with T2.1 |
| T1.5 | Surface **N1 vs N4** — narration and diagnostics compare commanded fpt to the matched band and say so | `narrate.rs:426-427`, `from_tool_load.rs` | fixture with commanded fpt 5× band max must produce a visible line; today it produces none |
| T1.6 | Record the LUT scale factors and the `pass_role` mismatch on every verdict, not only extrapolated ones | `chipload.rs:473-488`, `verdict.rs` | a 0.99×-scaled row and a 0.38×-scaled row must be distinguishable in the report |
| T1.7 | Add the **test-only feed-explanation snapshot assembler** the plan authorises, driving the B3 fixture end-to-end | `crates/rs_cam_core/tests/` | reproduces §4.3's arithmetic from live code rather than by hand |

### Tier 2 — structural parity (same numbers, one owner)

| id | change | rationale |
|---|---|---|
| T2.1 | Extract `feed_per_tooth(feed, rpm, flutes)` as the single owner and route all 7 C-1 sites through it | C-1; algebra is already identical, so this is provably number-preserving |
| T2.2 | Delete the inlined `op_family_to_lut` at `gcode/mod.rs:474-483` in favour of the helper | C-3/P-12; arms are identical today |
| T2.3 | Route Suggest's LUT lookup through `matched_chip_envelope` (or make the two entry points share one resolver with an explicit `allow_rpm_only` flag) | P-3/F-6. **Number-moving if a different row wins** — measure first, then decide; the *measurement* is tier 2, the *switch* is tier 3 |
| T2.4 | Give `derate_chipload_bounds` a typed `DocRatioBasis` argument so the three denominators are declared at the call site rather than implied | C-5/P-5 |
| T2.5 | Declare which axial field each gate consumes; add a parity sentry that `axial_doc_mm` and `axial_engagement_mm` agree for lateral samples or that the choice is deliberate | P-7 |
| T2.6 | Add the RPM/power/deflection **parity sentry** plan §H1 gate 3 requires: same canonical inputs → Suggest projection and gate projection agree once observed-only inputs are substituted with the same assumptions | gates 2 and 3 |
| T2.7 | Add the **tapered shallow-DOC matrix** (ball / taper / V-bit × DOC sweep) plan §H1 gate 4 requires | protects C3 |
| T2.8 | Add the **one end-to-end fixture** where row id, provenance, extrapolation status, advisory, verdict and diagnostic wording must all agree | gate 5 |
| T2.9 | Scale `ae_*_mm` / `ap_*_mm` alongside chipload in `build_result`, **or** document why they must not be — and make the D9 arc read the correct one | P-15. Number-moving via D9 ⇒ measure in tier 2, switch in tier 3 |

### Tier 3 — numeric / operator-visible (Checkpoint B required)

Each row lists what would move and by how much, from the analysis above.

| id | proposal | who moves, by how much | evidence needed |
|---|---|---|---|
| T3.1 | **Resolve F-1**: either convert the gate's observation to advance-per-tooth (`÷ mean_chip_factor(lut_arc)`), or convert the band to arc-mean chip (`× f_lut`), or keep both and compare in a declared third unit | every chipload verdict on an `ae`-bearing row moves by `f_lut` — **4.3×** on a typical Amana roughing row (`f_lut ≈ 0.233`), **12.4×** on the live scallop row. Direction depends on which side is converted. Verdicts will flip. | a primary source for the vendor convention; the B3 fixture; a full re-run of `predicted_feed_gates_f035`, `sim_chipload_invariant`, `wanaka_e2e_chipload_gate`, `chipload_formula_calibration`, CLI smoke baselines |
| T3.2 | **Resolve F-2**: apply `machine.safety_factor` to Suggest's available power | Suggest's power-limit factor tightens by `1/safety_factor` ≈ **1.25–1.33×**; feeds drop only where `power_limited` already fires | smoke baselines; `flat_12mm_adaptive2d_oak_power` cell |
| T3.3 | **Resolve C-12**: make the rubbing floor diameter-aware (or band-aware: `max(floor(D), band_min)`), or subordinate it to the band | affects tools where `0.025 > band_max` — on the live row the clamp target drops from 0.0250 to ≤0.00916, a **2.7× feed reduction** on that op | `_litmatrix_rubbing_floor_clamp`, `_litmatrix_rpm_only_lut_chipload`, `flat_3mm_pocket_softwood` (already at 0.020); a primary source for a diameter-dependent floor |
| T3.4 | **Resolve C-6/C-7**: pick one diameter and one hardness law | LUT bands move by `s^(1−0.61)` — **1.46×** at the live 0.38 scale, larger at extreme scales | primary sources; `_litmatrix_ipe_janka_scaling` re-pin |
| T3.5 | **Resolve C-13/F-5**: replace `arc_fit_ratio_for_op` with a projection of the gate's actual mechanism (`f_lut × expected_feed_ratio`) | only Adaptive3d and DropCutter feed-up currently fires; magnitude depends on T3.1 | recalibration against a post-T3.1 gate, on ≥2 fixtures |
| T3.6 | **Resolve P-5**: use the LUT-semantics diameter in `recompute_chipload_bounds_for_dpp` | up to **2×** band change on ball tools at shallow DOC after a DPP mutation | targeted fixture; Suggest smoke |
| T3.7 | **Resolve P-8/P-9**: one declared steady-state population across the three gates, keyed on the same feed the metric uses | peaks may move on kinematically-throttled paths | `lead_in_out_feed_rates_f040`, `adaptive_feed_modulation_pipeline_f036b` |
| T3.8 | **Resolve P-6**: one declared radial-width definition for power | ≈**1.23×** at 30 % radial | `flat_12mm_adaptive2d_oak_power` |
| T3.9 | **Resolve P-16**: one feed ceiling for Suggest and the optimizer | optimizer proposals above the cutting ceiling disappear | optimizer fixtures |

### Tier 4 — calibration decisions (Checkpoint B + `/refresh-lit-matrix`)

| id | question for the operator |
|---|---|
| T4.1 | **Is a vendor "chip load" column a linear advance per tooth, or a mean chip thickness?** Everything in T3.1 turns on this. Requires a primary source, not a code reading. |
| T4.2 | Should the chipload floor be absolute (tool-protection) or relative to the matched band (vendor-fidelity)? They conflict below ≈Ø2 mm. |
| T4.3 | What is the defensible diameter exponent for chipload — 1.0, 0.61, or per-family from the charts? |
| T4.4 | Should a `semi_finish` LUT row be allowed to win a `finish` query on score alone, or should pass role become a hard filter? (It did on the live B3 op.) |
| T4.5 | Should `is_extrapolated` carry direction and clamp-saturation, given B6 keeps advisory severity at `Info`? |
| T4.6 | Should `family_default_janka`'s synthetic anchor be flagged as weak provenance (which would route more low-side trips to advisory)? |
| T4.7 | Do the drill thresholds inherit any of the above, or are they independently sourced? — hand-off to **M2/R5**, which owns `DRILL_GATE_EVIDENCE_AUDIT.md`. |

**Ordering constraint.** T3.1 must precede T3.3, T3.5 and every drill
recalibration in M2, because it changes the axis those thresholds are expressed
on. T1.7 must precede T3.1, because a hand-checked arithmetic reconciliation is
not evidence a gate change can be re-measured against.

---

## 9. What could not be verified without the Cargo slot

The slot was held for the whole wave (W1 `standing_material_channel_am9`, then
`cargo test -p rs_cam_core --lib`, plus a foreign `spec-index` job). Per plan
§2 rule 10 this wave ran **zero** Cargo commands. The following are therefore
**NOT RUN — slot unavailable**, and no output for them is asserted anywhere
above:

| # | what | intended command |
|---|---|---|
| 1 | Live re-derivation of §4.3 from shipped code rather than by hand | the T1.7 snapshot assembler, `cargo test -p rs_cam_core --test <new>` |
| 2 | `mean_chip_factor(0.58565) = 0.080335` confirmed against `chip_geometry` rather than against `chipload.rs:109`'s mirror | `cargo test -p rs_cam_core --lib tool_load::chipload::` |
| 3 | Literature-matrix freshness report and citation audit at the real date | `LIT_MATRIX_TODAY=2026-08-04 cargo test -p rs_cam_core --test literature_matrix` |
| 4 | Current green/red state of `predicted_feed_gates_f035`, `chipload_advisory_disclosure_h4`, `sim_chipload_invariant`, `chipload_formula_calibration`, `wanaka_e2e_chipload_gate`, `tapered_width_model_parity_c3` | per-test `cargo test -p rs_cam_core --test <name>` |
| 5 | Whether the Ball-nose P-5 divergence is reachable in a shipped configuration (needs a DPP-mutating axial-envelope pass on a ball tool) | targeted fixture |
| 6 | The exact live `f_lut` for the *roughing* rows (the 0.233 figure is quoted from `chipload.rs:108`'s own docstring, not measured) | `--lib` unit probe |

Known reds inherited and **not** attributable to this wave: three
`adaptive3d` `--lib` tests and `wanaka_suggest_baseline` (environmental, plan
§2 rule 9).

---

## 10. Checkpoint B decision list

The operator is asked to rule on the following, in this order. Everything above
tier 2 is blocked until item 1 is answered.

1. **T4.1 — the unit question.** Is the vendor chipload column an advance per
   tooth or a mean chip thickness? This is a literature question, not a code
   question, and it determines whether F-1 is a 4–12× bug or a documentation gap.
2. **T3.1 — which side converts**, and therefore which verdicts flip.
3. **T3.2 — Suggest's power safety factor.** Small, isolated, unambiguous
   direction (more conservative). Could be approved independently of item 1.
4. **T3.3 / T4.2 — the rubbing floor.** Approve a diameter- or band-aware floor,
   or accept the documented contradiction with an owner.
5. **T2.3 / T3.x — one LUT resolver for Suggest and the gate.** Approve
   measuring the row-selection delta first (tier 2), then the switch.
6. **T3.4 / T4.3 — one diameter and one hardness law**, with source lineage into
   `CREDITS.md`.
7. **T4.4 — pass role: filter or score?**
8. **T3.5 — retire or re-key `arc_fit_ratio_for_op`** once item 2 is settled.
9. **Scope confirmation:** T1.1–T1.7 (report-only) may proceed under PR-7 now;
   T2.1–T2.9 (structural, number-preserving) may proceed after item 1 is
   answered even if items 2–8 remain open.
10. **Hand-off:** T4.7 drill questions transfer to M2/R5 rather than expanding
    this wave.

---

## Appendix A — grep index

```
feeds/mod.rs:684    RUBBING_FLOOR_MM_TOOTH = 0.025
feeds/mod.rs:702    drill_rpm_envelope_for_diameter
feeds/mod.rs:748    milling_rpm_ceiling_for_diameter
feeds/mod.rs:763    calculate            (the whole Suggest projection)
feeds/mod.rs:783    effective_d  #1 = engaged_diameter_at_doc(commanded DOC)
feeds/mod.rs:835    formula chipload  k0·D^0.61·(1/h)^1.26
feeds/mod.rs:852    find_best_row_for_geometry     ◄── C-4 Suggest branch
feeds/mod.rs:890    chipload_doc_ratio (denominator = #1)
feeds/mod.rs:1168   effective_d  #3 = effective_diameter(...)  (shadows #1)
feeds/mod.rs:1190   raw_feed = rpm · chip · flutes · thinning · depth_tier
feeds/mod.rs:1233   available_power = power_at_rpm(rpm)   ◄── F-2, no safety_factor
feeds/mod.rs:1341   rubbing-floor clamp
feeds/mod.rs:1376   drill plunge-envelope clamp + RPM follow-down
feeds/mod.rs:1735   effective_diameter (chip-thinning semantics)
feeds/geometry.rs:143  doc_derating_scale
feeds/geometry.rs:232  derate_chipload_bounds
feeds/predict.rs:633   predict_observed_chipload_mm
feeds/predict.rs:735   arc_fit_ratio_for_op          ◄── C-13
feeds/suggest.rs:1433  recompute_chipload_bounds_for_dpp   ◄── P-5
feeds/suggest.rs:1465  enforce_invariants (pass order)
feeds/suggest.rs:2059  recalibrate_feed_for_chipload
feeds/vendor_lookup.rs:181  find_best_row_for_geometry
feeds/vendor_lookup.rs:206  find_best_chip_envelope_row  ◄── C-4 gate branch
feeds/vendor_lookup.rs:265  diameter_scale_factor  (^1.0)
feeds/vendor_lookup.rs:291  family_default_janka
feeds/vendor_lookup.rs:306  hardness_scale_factor  (^1.0)
feeds/vendor_lookup.rs:349  is_extrapolated SET  (|ln(total)| > ln 1.4)
feeds/vendor_lookup.rs:352  chipload scaled;  :355-363 rpm/ap/ae NOT scaled
feeds/vendor_lookup.rs:459  passes_must_match   (no hardness clause)
feeds/vendor_lookup.rs:500  score_observation
feeds/force.rs:86/109/122   immersion_angle / affine_coefficients / lateral_cutting_force
tool_load/mod.rs:63    effective_feed_for_sample
tool_load/mod.rs:199   chipload_envelopes_for_session
tool_load/chipload.rs:69   matched_chip_envelope
tool_load/chipload.rs:109  mean_chip_factor
tool_load/chipload.rs:127  lut_nominal_arc_rad
tool_load/chipload.rs:232  steady_state_samples_for_toolpath (95 % filter)
tool_load/chipload.rs:430  gate doc_ratio (denominator = #2)
tool_load/chipload.rs:455  ChipBoundsSource classification
tool_load/chipload.rs:553  predicted-feed ratio scale
tool_load/chipload.rs:563  D9 arc normalisation  ◄── F-1
tool_load/chipload.rs:620  median statistic
tool_load/chipload.rs:698  burn advisory downgrade (F3.3)
tool_load/power.rs:68      predicted_power_kw
tool_load/power.rs:194     arc-slab radial width
tool_load/power.rs:214     available × safety_factor   ◄── F-2 counterpart
tool_load/deflection.rs:80 sample_tip_deflection_mm
tool_load/locality.rs:149  is_steady_state_for_gate
tool_load/verdict.rs:606   ChipBoundsSource
tool_load/verdict.rs:632   low_side_is_advisory
tool_load/verdict.rs:651   row_id
tool_load/optimize/context.rs:123  delegates to matched_chip_envelope
tool_load/optimize/retarget/chipload.rs:109  multiplier = target / gate-observed
dexel_stock/simulation.rs:496  Engagement field assignment  ◄── F-4
dexel_stock/simulation.rs:581  effective_chip_thickness_mm (returns the MEAN)
dexel_stock/stamping.rs:786    chipload_mm_per_tooth (only extracted C-1 helper)
narrate.rs:426             narration nominal   ◄── N1
gcode/mod.rs:438           project_load_report;  :474 inlined op-family map
diagnostics/adapters/from_tool_load.rs:119  advisory disclosure (H4)
simulation_cut.rs:88-96    Engagement chip-thickness doc comments  ◄── F-4
```

## Appendix B — the B3 fixture, specified but not built

Per plan §H1 acceptance gate 1 the fixture must emit four explicitly named
values and explain every delta. Specification, for PR-7:

- **Tool:** tapered ball, tip Ø1.0, taper 5.26°, shank Ø6 (the wanaka geometry,
  synthesised in-test — **not** loaded from `wanaka.toml`).
- **Material:** `SolidWood { HardMaple }` or whichever species lands the
  hardness scale at 1.00 against the row's 1450 Janka.
- **Row:** `amana-tapered-hardwood-scallop-3175-2f` from the embedded LUT.
- **Op:** `Scallop`, `pass_role = Finish`, commanded feed/RPM/flutes chosen so
  `feed/(rpm·flutes) = 0.0714`.
- **Trace:** hand-built (the `axial_doc_step_multiple_h4.rs` pattern) so no
  generator, arc fit, lead-in or depth planner participates. Populate
  `predicted_feeds` at 0.1282 × commanded on every move.
- **Asserts (pre-registered):** the four values are emitted under four distinct
  names; `N3 / (N1 · f_lut · feed_ratio)` is within 1 %; the report names
  `f_lut` and `feed_ratio` explicitly; `N1 / N4_max` is surfaced; the verdict
  stays `Within` with a `burn_advisory` and `row_id "vendor_lut_extrapolated"`.
- **Non-vacuity:** with `predicted_feeds` empty the fixture must produce a
  materially different N3, proving the feed factor is live.
