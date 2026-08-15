# X-VAC — the empty-population gate vacuity census

Date: 2026-08-14
Wave: TD3 sweep pool **S-1**
Ledger row: **X-VAC** (`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4)
Programme rule: §0.4 — *"Populations at source intent; a gate handed an
empty population is vacuous until its `sample_count`/`sample_range` is
checked."*
Revision: branch `tech-debt-3`, base `9c2eb01e`
Sentry: `crates/rs_cam_core/tests/gate_population_vacuity_xvac.rs`

---

## 0. What was filed, and what this wave adds

The ledger row, verbatim:

> **The empty-population gate vacuity class.** Measured on arc-fit: three
> gates returned `Within` with `sample_range 0..0`, no locality and
> `available_kw 0.0` — indistinguishable on every surface from a measured
> clean cut, and one of them was suppressing a burn advisory. The arc-fit
> *instance* is fixed; **the class is not**. Any predicate that empties a
> gate's population produces a healthy-looking pass, and nothing on any
> surface distinguishes it. […] Note that a *verdict* bar tests this
> vacuously — the bar must be a **population** bar.

A-9's `CHIP_THICKNESS_POLICY.md` §2.1 already enumerated **thirteen silent
emptying paths** for one population — the chip-thickness signal — in three
tiers, and measured that the `sim_measurability` detector covers **2 of
13**. Those thirteen are **cited, not re-derived**; §2 below maps them
onto the gates that actually ship.

This wave generalises to **every gate that publishes a `LoadState`**, and
lands the marker: a typed `GatePopulation` on the verdict, rendered on
every verdict-bearing surface.

---

## 1. The census

Six gates ship a `LoadState` through `ToolpathLoadVerdict::criteria()`.
`plunge_stress` is a seventh guardrail but is **not** verdict-tier
(§1.4).

Legend for **Empty ⇒ visible?**: **REFUSES** = the gate returns
`Unmodeled` with a stated reason, which every surface already renders;
**SILENT** = the gate returns `Within` and, before this wave, nothing on
any surface distinguished it from a measured clean cut.

### 1.1 Milling gates — the sample pipeline

Every milling gate walks `trace.samples` and drops samples through the
same-shaped ladder. Numbering matches the order the predicates execute.

| # | predicate | site | chipload | power | deflection | empties population when |
|---|---|---|---|---|---|---|
| 1 | `toolpath_id` mismatch | each gate's loop head | selection | selection | selection | — (not a filter, a selector) |
| 2 | `!is_cutting` | `chipload.rs:314`, `power.rs:189`, `deflection.rs:216` | ✓ | ✓ | ✓ | the op never contacts material |
| 3 | air-cut, `radial_woc_fraction < 0.02` | same lines | ✓ | ✓ | ✓ | radial channel blind (§2 paths 5/6) or a genuine air pass |
| 4 | validity: `arc_engagement_radians.is_none()` (power/defl) / `effective_chip_thickness_mm.is_none()` (chipload, **F-VALID**, vestigial) | `power.rs:195`, `deflection.rs:231`, `chipload.rs:771` | ✓ | ✓ | ✓ | `capture_arc_engagement == false`, or the cutter declines the geometry (§2 paths 1/7) |
| 5 | steady-state feed, `>= 0.95 × commanded` | `chipload.rs:306,320` | ✓ | — | — | a wholly kinematically-throttled op |
| 6 | `radial_width <= 0.0` | `power.rs:207` | — | ✓ | — | zero engagement radius at the sampled DOC |
| 7 | `sample_tip_deflection_mm(..) == None` | `deflection.rs:239` | — | — | ✓ | the deflection model declines the cutter/DOC |
| 8 | `achieved_feed_per_tooth_mm == None` (no `rpm × flutes` divisor) | `chipload.rs:785` | ✓ | — | — | samples cannot state their own spindle |
| 9 | **`locality::is_phantom_transit`** | `chipload.rs:817`, `power.rs:234`, `deflection.rs:249` | ✓ | ✓ | ✓ | every cutting sample sits in `LeadOut` / `LinkBridge` / `DressupArtifact` / `WaterlineCleanup` — **or**, with `spans == None`, in *any* `in_transit_span` |
| 10 | **`locality::is_configured_entry`** | `chipload.rs:820`, `power.rs:237`, `deflection.rs:252` | ✓ | ✓ | ✓ | every cutting sample sits under an `Entry` span (a pure plunge/helix-entry pass) |

**The visibility split is decided by where each gate sets its "we saw
something" flag**, and that is the whole defect:

| gate | flag | set at | ⇒ predicates 2–5 empty | ⇒ predicates 6–10 empty |
|---|---|---|---|---|
| chipload | `any_in_cut` (`chipload.rs:506`), then `steady_samples.is_empty()` (`:514`), then `valid_count` (`:797`) | before 9/10 | **REFUSES** — `AllSamplesAirCutOrRapid` / `SimulationRequired` / `SteadyStateSamplesNotPresent` / `ArcEngagementNotCaptured` / `CutterModeUnsupported` | **SILENT** (was) |
| power | `any_arc_captured` (`power.rs:198`) | before 6/9/10 | **REFUSES** — `ArcEngagementNotCaptured` | **SILENT** (was) |
| deflection | `any_arc_captured` (`deflection.rs:225`) | before 7/9/10 | **REFUSES** — `ArcEngagementNotCaptured` | **SILENT** (was) |

So the class is precisely: **a predicate that runs *after* the gate has
already decided the trace is usable**. There are **eight** such
gate×predicate pairs (chipload 9,10; power 6,9,10; deflection 7,9,10).

Two riders on predicate 9 that make it far likelier than it reads:

- With `span_lookup == None`, `is_phantom_transit` degrades to the bare
  `sample.in_transit_span` flag (`locality.rs:219`) and
  `is_configured_entry` returns `false` (`:243`) — **every** transit
  sample is dropped, with no Entry split. Any consumer that evaluates
  gates without threading spans is on this path.
- `sample_count` on the chipload verdict is `valid_count`, which
  increments at `chipload.rs:797` **before** the predicate-9 skip. The
  code says so verbatim at `:790-798`: it *overstates* the gate's
  denominator. An agent checking `sample_count` to test for vacuity gets
  the wrong number — which is why the new marker publishes
  `burn_samples.len()` and not `valid_count`.

### 1.2 What each milling gate returns when predicates 6–10 empty it

| gate | verdict | headline numbers | evidence | confidence |
|---|---|---|---|---|
| chipload | `Within` | `approach_to_max.observed = 0.0`, `approach_to_min = None`, `burn_advisory = None`, `ceiling_advisory = None` | `SampleEvidence::empty()` → `0..0`, no locality | the matched row's — unchanged |
| power | `Within` | `peak_kw = 0.0`, **`available_kw = 0.0`** | `0..0`, no locality | `Approximate` |
| deflection | `Within` | `peak_mm = 0.0` | `0..0`, no locality | **`Validated`** |

The deflection row is the worst of the three and was not in the ledger:
with `peak_delta_mm == 0.0` and `any_slot == false`, `deflection.rs:335`
resolves the confidence tier to `Confidence::Validated` — **the most
trustworthy label the gate can print, on a verdict resting on nothing.**
Pinned by `the_2026_08_05_shape_still_reproduces`.

`power`'s `available_kw = 0.0` is the ledger's own tell, and it is
reachable exactly as filed: `last_available_kw` is only assigned inside
the post-predicate-9 block (`power.rs:251` after this wave's edit), so an
all-transit toolpath reports a machine with no power.

### 1.3 Drill gates — the hole pipeline

The drill trio has no sample stream at all; its population is the hole
set on `DrillOp`.

| gate | observation | empties when | empty ⇒ |
|---|---|---|---|
| chip welding | `summary.chip_welding_dtd` | `holes` empty, or every hole `top_z == bottom_z` → `deepest = 0.0` (`drill_metrics.rs:319-328`) → `max_dtd = 0.0` → `ChipWeldingRisk::Low` | **SILENT** `Within { observed: 0.0 }` |
| peck adequacy | `summary.per_peck_max_dtd` | same — `per_peck_max_depth_to_diameter_of(cycle, 0.0, d) = 0.0` | **SILENT** `Within { observed: 0.0 }` |
| plunge feed | `feed_rate / diameter` (config only) | never empties *its own* input, but with no hole of positive depth it judges a feed **nothing runs at** | **SILENT** `Within` |

`hole_count` and `deepest_hole_index` both existed on
`DrillToolpathSummary` and **neither reached the verdict**;
`worst_hole_id: None` was the only trace, and it is
`skip_serializing_if = "Option::is_none"`, so on the wire it simply
disappears. Three gates, one shared population — they are vacuous
together, which is why the marker sits once on `DrillGatesVerdict`.

### 1.4 Not verdict-tier (checked, out of scope)

- **`tool_load::plunge_stress`** returns `Option<PlungeStressWarning>`
  from tool geometry + configured plunge rate. No population, no
  `LoadState`, no `Within`. A `None` means "no cap for this geometry" or
  "under the cap" — the two are conflated, which is a *different* defect
  class and is not X-VAC.
- **`chipload_envelopes_for_session`** (`tool_load/mod.rs:289-301`) is
  not a gate but is worth recording: it folds `axial_doc` over
  `is_steady_state_for_gate(s, **None**)` — the hardest fallback, where
  every `in_transit` sample is phantom. If that population empties,
  `axial_doc` silently falls to `0.0` and the LUT is queried at the
  tool's *tip* diameter. That is a silent population collapse that moves
  the **band** rather than the verdict, and it feeds the viewport
  heat-map. Not fixed here; recorded for whoever next touches F-HEATMAP.

### 1.5 Where `sim_measurability` sits

`sim_measurability::{Measurability, SimMetric, MeasurabilityReason}` is
real, wired, and **reaches no `tool_load` gate**. Its only production
consumer is the air-cut threshold gate
(`session/compute.rs:4013-4023`, `air_cut_offenders_for_toolpaths`),
plus narration (`narrate.rs:1914`), the CLI report and the GUI strip.

A-9 measured the reach precisely: `classify_engagement`'s blindness
predicate is `s.engagement.radial_woc_fraction > 0.0 { continue; }`
(`sim_measurability.rs:414`) — *not* "the metric is `None`" — so it
catches the fresh-material floor and the perp-coverage gate and
**nothing else**, i.e. **2 of the 13** chip-thickness paths. It has
nothing to say about predicates 6–10 above, all of which are
locality/model refusals with healthy `radial_woc_fraction`.

**Conclusion: the measurability abstention and the population marker are
complementary, not overlapping.** Measurability answers *"could this
simulation see the quantity?"*; the population marker answers *"how many
readings actually reached the comparison?"* A gate can be `Measurable`
and vacuous at the same time, which is exactly the 2026-08-05 shape.

### 1.6 Headline

| | count |
|---|---|
| gates publishing a `LoadState` | **6** |
| distinct predicates that can shrink a milling gate's population | **10** |
| gate × predicate pairs where an empty population **REFUSES** (already visible) | 11 (chipload 2,3,4,5,8; power 2,3,4; deflection 2,3,4) |
| gate × predicate pairs where an empty population was **SILENT** | **8** (chipload 2, power 3, deflection 3) |
| drill gates silently vacuous on a depth-less hole set | **3** (one shared population) |
| **silent emptying paths total, pre-wave** | **11** |
| surfaces on which a vacuous verdict was distinguishable, pre-wave | **0** |

---

## 2. A-9's thirteen, mapped (cited, not re-derived)

`CHIP_THICKNESS_POLICY.md` §2.1 tiers them. Their relationship to the
gates that ship:

| A-9 tier | paths | reaches a shipped gate as |
|---|---|---|
| Tier 1 (trace-wide): `capture_arc_engagement == false` | 1 | predicate **4** → all three milling gates **REFUSE** (`ArcEngagementNotCaptured`). Visible. |
| Tier 2 (production, per sample): non-cutting emitter; degenerate segment; `Plunge` kinematics; fresh-material floor; perp-coverage gate; cutter shape guards; `max_penetration` accumulation | 2–8 | predicates **2/3/4** → **REFUSE** if they empty the whole trace; otherwise they *shrink* the population without emptying it, which the new `offered` vs `contributing` pair now measures. |
| Tier 3 (consumption): `!is_cutting`, air-cut, steady-state feed, phantom transit, configured entry | 9–13 | predicates **2/3/5/9/10** — and 9/10 are **exactly the silent pairs** §1.1 isolates. |

A-9 said the measurability detector covers 2 of 13 (paths 5 and 6). This
wave does not widen that; it makes the **result** of any of the thirteen
visible on the verdict, whichever one fired.

---

## 3. The marker

### 3.1 Design and its bar

Bar, from the ledger: *a vacuous verdict must be distinguishable on every
surface that renders a verdict*, and the bar itself must be a
**population** bar. Explicitly **not** a threshold change and **not** a
verdict flip.

```rust
// crates/rs_cam_core/src/tool_load/verdict.rs
pub struct GatePopulation {
    pub contributing: usize,   // reached the gate's own comparison
    pub offered: usize,        // handed to the gate before its filters
    pub unit: PopulationUnit,  // Samples | Holes
}
impl GatePopulation {
    pub fn is_vacuous(self) -> bool { self.contributing == 0 }
    pub fn filtered_out(self) -> usize;
    pub fn vacuity_clause(self) -> String;  // one wording, all surfaces
}
```

Placement follows the **existing verdict-evidence shape** rather than
adding a parallel one:

- `SampleEvidence.population: Option<GatePopulation>` — `SampleEvidence`
  is already on `PowerVerdict::{Within,Exceeds}`,
  `DeflectionVerdict::{Within,Exceeds}` and every `ChiploadMetric`, so
  both arms of all three milling gates carry it with **two** struct
  literals in the tree to update, not ~290 enum-variant sites.
- `DrillGatesVerdict.population: Option<GatePopulation>` — one shared
  hole population, fanned onto all three drill criteria by
  `DrillGateOutcome::as_criterion_status`.
- `CriterionStatus.population` + `is_vacuous()` + `vacuity_clause()` —
  the single field every renderer reads.

Three deliberate contract choices:

1. **`Option`, and `None` means NOT STATED — never zero.** Same contract
   the `ToolpathStats` report-only findings carry. A pre-2026-08-14 wire
   payload with no `population` key must not start reading as an empty
   gate. Pinned by `an_unstated_population_is_unknown_not_empty`.
2. **Never infer vacuity from `sample_range`.** `0..0` already carries a
   second, legitimate meaning ("no single sample is worth naming") — the
   ambiguity X-VAC *is*. `is_vacuous()` reads the population and nothing
   else.
3. **The chipload marker publishes `burn_samples.len()`, not
   `valid_count`.** `valid_count` is documented at its own definition as
   overstating the denominator (it increments before the transit skip).
   Publishing it as the vacuity marker would reproduce the defect inside
   the fix.

**Report-tier, and pinned as such.** `LoadState`, `ExceededCriterion`,
`exceeded_criteria()` and the export gate are byte-identical across this
change; a vacuous `Within` is still `Within`, it just says it is vacuous.
`gate_outcome_is_untouched_by_the_marker` asserts it. Whether a vacuous
gate should *refuse* is a Checkpoint question and is **not taken here**.

### 3.2 What each gate now publishes

| gate | `contributing` | `offered` | unit |
|---|---|---|---|
| chipload | `burn_samples.len()` — the trip set the median and the in-range peak are both drawn from | `steady_samples.len()` | samples |
| power | samples reaching the peak comparison (post predicates 6/9/10) | every sample with this `toolpath_id` | samples |
| deflection | samples reaching the peak comparison (post predicates 7/9/10) | every sample with this `toolpath_id` | samples |
| drill ×3 | holes with `top_z − bottom_z > 0` | `holes.len()` | holes |

### 3.3 Per-surface rendering

One clause, authored once in core (`GatePopulation::vacuity_clause`), so
the wording cannot drift between crates:

> ` — VACUOUS: this verdict rests on 0 of 12 samples (all filtered out); it is not a measurement of a clean cut`

| # | surface | site | before | after |
|---|---|---|---|---|
| 1 | diagnostics adapter — the construction site shared by the CLI `project` report, the GUI diagnostics panel, MCP `get_diagnostics` and the narration list | `diagnostics/adapters/from_tool_load.rs` (`vacuity_clause` helper; all 6 gate arms + both drill arms) | `Power within budget (0.00/0.00 kW peak)` | same + the clause. Id, severity, state, count unchanged (`LOAD_CHIPLOAD_WITHIN` is what the supersession reducer keys on). |
| 2 | MCP `get_tool_load_report` | `rs_cam_viz/src/app/mcp.rs:2880` serialises the report whole (`serde_json::to_value(&report)`) | `evidence: {sample_range:{start:0,end:0}}` — byte-identical to a measured light cut | `evidence.population = {contributing:0, offered:12, unit:"samples"}` |
| 3 | GUI verdict badge + tooltip | `rs_cam_viz/src/ui/sim_diagnostics.rs:1113` / `:1140` | `theme::SUCCESS`, text `0%` or `OK` | dimmed `∅` badge; tooltip leads with the clause instead of "Within bounds — validated" |
| 4 | GUI per-toolpath status flags | `rs_cam_viz/src/ui/sim_op_list.rs:990` / `criterion_detail` | **no flag at all** — `Within` + `Validated` is the silent case, so a gate that measured nothing looked *cleaner* than one that measured an approximation | `∅ <gate>` flag at rank 2, detail leads with the clause |

**Surface deliberately not changed:** `rs_cam_cli/src/smoke.rs` writes
`chipload_kind` / `deflection_kind` / `power_kind` into
`planning/toolpath_acceptance/baselines/2026-06-04.csv`. Adding a column
or widening a value there is a baseline re-pin, which **F-BASE** already
owns and which §0.2 says must carry old/new and a consumer census. The
verdict *kinds* it records are unchanged by this wave, so the file stays
byte-stable. Recorded here so the gap is not mistaken for coverage.

**Surface unaffected by construction:** the g-code export gate
(`gcode/mod.rs`) fires on `Exceeds` only, and a vacuous gate never
exceeds.

### 3.4 Red / green

`crates/rs_cam_core/tests/gate_population_vacuity_xvac.rs`, 9 tests.

Red-first was measured, not asserted: `SampleEvidence::with_population`
was temporarily neutered to a no-op (the marker not stated, everything
else identical) and the suite re-run.

**RED (marker neutered) — 5 of 9 fail:**

```
test the_2026_08_05_shape_still_reproduces ... ok       <- passes both ways, by design
test gate_outcome_is_untouched_by_the_marker ... ok     <- passes both ways, by design
test every_per_sample_gate_states_its_population ... FAILED
        Power stated no population
test the_diagnostics_surface_says_the_pass_is_vacuous ... FAILED
        every vacuous gate row must say so, got: Power within budget (0.00/0.00 kW peak)
test the_mcp_wire_carries_the_population ... FAILED
        assertion `left == right` failed
          left: Null
test an_unstated_population_is_unknown_not_empty ... FAILED
        the marker must be on the wire to remove
test the_shared_renderer_clause_names_the_population ... FAILED
test result: FAILED. 4 passed; 5 failed
```

`Power within budget (0.00/0.00 kW peak)` is the pre-fix message
verbatim — the 2026-08-05 shape, rendered as a pass.

**GREEN (as landed):**

```
running 9 tests
test the_2026_08_05_shape_still_reproduces ... ok
test every_per_sample_gate_states_its_population ... ok
test an_unstated_population_is_unknown_not_empty ... ok
test the_diagnostics_surface_says_the_pass_is_vacuous ... ok
test the_mcp_wire_carries_the_population ... ok
test the_shared_renderer_clause_names_the_population ... ok
test the_drill_trio_states_its_hole_population ... ok
test the_drill_criteria_and_diagnostics_carry_the_hole_population ... ok
test gate_outcome_is_untouched_by_the_marker ... ok
test result: ok. 9 passed; 0 failed
```

Note which two tests pass in **both** columns:
`the_2026_08_05_shape_still_reproduces` (the pre-fix reproduction, kept
permanently per §0.1 — every part of the ledger's shape is still true,
because the fix added a field rather than moving a verdict) and
`gate_outcome_is_untouched_by_the_marker` (the report-tier guard). Those
two are the ledger's own warning made executable: **a verdict bar tests
X-VAC vacuously.** Every assertion that constitutes the bar reads
`population.contributing` or a string derived from it.

---

## 4. What is NOT fixed

1. **No gate refuses on a vacuous population.** Report-only, by design.
   Whether an empty gate should abstain (`Unmodeled`) rather than pass is
   number-moving and needs a Checkpoint ruling. If ruled yes, the natural
   shape already exists: a new `UnmodeledReason::PopulationEmpty` carrying
   the `GatePopulation`.
2. **`sample_count` on `FeedExplanation` still publishes `valid_count`**
   and still overstates the denominator (`chipload.rs:790-798`). It is a
   shipped wire field; moving it is its own gated change. The new marker
   sits beside it with the honest number.
3. **`chipload_envelopes_for_session`'s silent `axial_doc → 0.0`
   collapse** (§1.4) — a population emptying that moves a *band*, not a
   verdict, on the surface F-HEATMAP owns. Not marked.
4. **`plunge_stress`'s `None` conflation** — "no cap for this geometry"
   vs "under the cap". A different class; recorded, untouched.
5. **`sim_measurability` still reaches no `tool_load` gate** (§1.5). This
   wave does not wire it; the two mechanisms answer different questions
   and A-9's own §2.3 shows the detector would cover 2 of 13 paths if it
   were wired.
6. **The `smoke` baseline CSV** carries no vacuity column (§3.3), under
   F-BASE.

---

## 5. Files

| file | change |
|---|---|
| `crates/rs_cam_core/src/tool_load/verdict.rs` | `GatePopulation`, `PopulationUnit`, `SampleEvidence::{population, with_population, is_vacuous}`, `CriterionStatus::{population, is_vacuous, vacuity_clause}`, three `as_criterion_status` wirings |
| `crates/rs_cam_core/src/tool_load/power.rs` | `offered`/`contributing` counters; population on both arms |
| `crates/rs_cam_core/src/tool_load/deflection.rs` | same |
| `crates/rs_cam_core/src/tool_load/chipload.rs` | `offered` = steady-state set, `contributing` = `burn_samples.len()`; population on all four `ChiploadMetric` sites |
| `crates/rs_cam_core/src/tool_load/drill_gates.rs` | `DrillGatesVerdict::population` (holes with positive depth); `as_criterion_status` takes and forwards it |
| `crates/rs_cam_core/src/diagnostics/adapters/from_tool_load.rs` | shared `vacuity_clause` helper appended to all 6 gate messages + both drill arms |
| `crates/rs_cam_viz/src/ui/sim_diagnostics.rs` | `∅` badge + tooltip |
| `crates/rs_cam_viz/src/ui/sim_op_list.rs` | `∅` status flag + `criterion_detail` |
| `crates/rs_cam_core/tests/gate_population_vacuity_xvac.rs` | **new** — 9 tests |
