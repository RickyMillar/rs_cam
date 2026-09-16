# SURVEY_CORE — the load model data path in `rs_cam_core`

Read-only survey. Date: 2026-09-17. Scope: `crates/rs_cam_core` only.

## How to read a citation

Another session moves modules in this tree while this survey runs. The
**symbol** is the durable half of every citation. The path and the line
are a convenience and may rot within the day.

Every citation has this form:

> `Owner::symbol` (`path:line` at time of survey)

Search for the symbol. Use the path only as a starting point.

## Churn observed during this survey

The report re-verified every cited symbol at the end of the survey.
**No symbol changed its name, its file or its meaning.** Three line
numbers drifted. These files carried a modification time later than the
bulk of the tree, so the report re-read each one:

- `crates/rs_cam_core/src/tool_load/verdict.rs`
- `crates/rs_cam_core/src/tool_load/mod.rs`
- `crates/rs_cam_core/src/tool_load/plunge_stress.rs`
- `crates/rs_cam_core/src/feeds/predict.rs`
- `crates/rs_cam_core/src/feeds/mod.rs`

Three citations needed correction:

| Symbol | First cited as | Correct at hand-off | Cause |
|---|---|---|---|
| the gantry travel-rate comment | `feeds/mod.rs:2221` | `feeds/mod.rs:2224` | **drift during the survey** |
| `PlungeStressWarning` | `plunge_stress.rs:45` | `plunge_stress.rs:47` | **drift during the survey** |
| `FeedsResult::chipload_bounds` | `feeds/mod.rs:465` | `feeds/mod.rs:472` | the report cannot separate its own mis-citation from drift |

`crates/rs_cam_core/src/feeds/mod.rs` and
`crates/rs_cam_core/src/tool_load/plunge_stress.rs` each gained lines
above the cited points between the first read and the re-verification.
Two line numbers moved by 3 and by 2 within about forty minutes.
**Treat every line number in this document as approximate. Treat the
symbol as exact.**

The brief stated that `simulation_cut.rs` had already moved into
`stock/` before this survey began. The report never saw the old path and
cites only `crates/rs_cam_core/src/stock/simulation_cut.rs`.

The report marks an inference with **(inferred)**. The report marks a
gap with **(not determined)**.

---

## 1. Per-sample data available, per metric

The per-sample record is `SimulationCutSample`
(`crates/rs_cam_core/src/stock/simulation_cut.rs:163`). The engine emits
one sample per subsegment of a move. `estimate_sample_count`
(`crates/rs_cam_core/src/dexel_stock/simulation.rs:1609`) documents the
subdivision rule: a move becomes
`max(⌈len/sample_step⌉, ⌈|Δz|/0.02⌉)` subsegments. The Z-drop divisor is
`ESTIMATOR_MAX_SUBSEGMENT_Z_DROP_MM`
(`dexel_stock/simulation.rs:1584`).

### 1.1 The time weight

Two time fields exist on every sample:

- `SimulationCutSample::cumulative_time_s` (`simulation_cut.rs:168`) —
  the clock at this sample.
- `SimulationCutSample::segment_time_s` (`simulation_cut.rs:169`) — the
  duration this sample represents.

`segment_time_s` is the time weight. The existing accumulators already
multiply by it. See `KinematicsAccumulator::radial_woc_time_weighted_sum`
(`simulation_cut.rs:1316`) and its siblings at
`simulation_cut.rs:1318`, `:1329`, `:1331` and `:1334`.

**A time-weighted distribution is therefore buildable from stored data.**

### 1.2 Chip thickness

Four distinct quantities exist. Do not mix them.

| Quantity | Symbol | Line | Optional |
|---|---|---|---|
| Commanded advance per tooth | `SimulationCutSample::chipload_mm_per_tooth` | 191 | no |
| Arc-mean chip thickness, gate input | `SimulationCutSample::effective_chip_thickness_mm` | 193 | yes |
| Arc-mean chip thickness, engagement vector | `Engagement::mean_chip_thickness_mm` | 103 | yes |
| Arc-peak chip thickness | `Engagement::peak_chip_thickness_mm` | 115 | yes |

All four live in `crates/rs_cam_core/src/stock/simulation_cut.rs`.
`Engagement` is the struct at `simulation_cut.rs:70`.

The doc on `Engagement::mean_chip_thickness_mm` states the commanded and
the arc-mean quantities differ by
`(2/arc)·(1 − cos(arc/2))·sin(arc)` below full slotting. The factor is
about 0.373 at half immersion.

The chipload gate reads `effective_chip_thickness_mm`. See the module
header of `crate::tool_load::chipload`
(`crates/rs_cam_core/src/tool_load/chipload.rs:22`) and the skip at
`chipload.rs:772`.

**Enough for a distribution: yes.** The value is per sample. The sample
carries `segment_time_s`.

### 1.3 Spindle power

**No power value exists on the sample.** The report read every field of
`SimulationCutSample` (`simulation_cut.rs:163-242`). No field names
power, torque or watts.

The power gate computes power per sample at gate time:

- `crate::tool_load::power::evaluate`
  (`crates/rs_cam_core/src/tool_load/power.rs:261`) — the gate.
- `crate::tool_load::power::predicted_power_kw`
  (`power.rs:256`) — the model. It is `pub(crate)`.
- `PowerModelInputs` (`power.rs:107`) — the model inputs:
  `kc_n_per_mm2`, `cross_section_mm2`, `axial_doc_mm`, `immersion_rad`,
  `engagement_diameter_mm`, `spindle_rpm` and `flute_count`.

The sample loop is at `power.rs:372`. The call is at `power.rs:418`.
The gate reads every input from the sample, the tool and the material.

**Enough for a distribution: yes, but only after re-computation.** The
inputs are per sample. A distribution pass must call
`predicted_power_kw` again, or the emitter must store the result.

### 1.4 Axial depth of cut

Three separate axial fields exist. The engine split them deliberately.

- `SimulationCutSample::axial_doc_mm` (`simulation_cut.rs:178`) — the
  legacy wire name. A pure-vertical plunge reports `0.0` here.
- `SimulationCutSample::axial_engagement_mm` (`simulation_cut.rs:182`) —
  the lateral, arc and helix axial engagement. The deflection gate and
  the chip-geometry gates read this axis.
- `SimulationCutSample::plunge_descent_mm` (`simulation_cut.rs:187`) —
  the Z descent of a pure-vertical plunge sample.

`Engagement::axial_doc_fraction` (`simulation_cut.rs:84`) holds the same
quantity as a fraction of flute length. It is `Option<f64>`. `None`
means **not measured**. `Some(0.0)` means measured and zero. The doc on
that field records the change from a `0.0` sentinel to the `Option`.

**Enough for a distribution: yes.** All three are per sample.

### 1.5 Deflection

**No deflection value exists on the sample.** The gate computes it:

- `crate::tool_load::deflection::sample_tip_deflection_mm`
  (`crates/rs_cam_core/src/tool_load/deflection.rs:80`) — the per-sample
  predictor. It reads `Engagement::radial_woc_fraction`,
  `SimulationCutSample::axial_engagement_mm` and
  `SimulationCutSample::arc_engagement_radians`.
- `crate::feeds::predict::tip_deflection_from_engagement`
  (`crates/rs_cam_core/src/feeds/predict.rs:419`) — the model it calls.
- `crate::tool_load::deflection::evaluate` (`deflection.rs:116`) — the
  gate. Its sample loop is at `deflection.rs:211`.

**Enough for a distribution: yes, but only after re-computation.** The
same condition as power applies.

### 1.6 Gantry push force

**No force value exists on the sample.** No force value exists anywhere
on the trace. See section 3.5.

### 1.7 Other per-sample fields a distribution needs

All on `SimulationCutSample` in
`crates/rs_cam_core/src/stock/simulation_cut.rs`:

- `is_cutting` (`:170`) — the in-cut predicate.
- `in_transit_span` (`:217`) — the transit flag. The existing peak
  metrics skip transit samples to avoid lift-bridge artifacts. The doc
  on that field names `peak_axial_doc_mm` and
  `peak_chipload_mm_per_tooth` as the metrics that skip.
- `cut_kinematics` (`:172`) — a `CutKinematics`
  (`simulation_cut.rs:26`): `Linear`, `Plunge`, `Helix`, `Arc`, `Rapid`.
- `toolpath_id` (`:164`) and `move_index` (`:165`).
- `span_path` (`:204`) and `source_intent` (`:242`).
- `feed_rate_mm_min` (`:173`), `spindle_rpm` (`:174`),
  `flute_count` (`:175`).
- `arc_engagement_radians` (`:189`), `removed_volume_est_mm3` (`:201`),
  `mrr_mm3_s` (`:202`).

One helper resolves which feed a gate evaluates against:
`crate::tool_load::effective_feed_for_sample`
(`crates/rs_cam_core/src/tool_load/mod.rs:66`). Its doc states that
every per-sample gate MUST call it rather than read `feed_rate_mm_min`
directly.

---

## 2. What the current summaries keep, and what they discard

### 2.1 The summary structs, by name

Five published summary structs live in
`crates/rs_cam_core/src/stock/simulation_cut.rs`:

| Struct | Line | Scope |
|---|---|---|
| `KinematicsSummary` | 373 | one `CutKinematics` class |
| `SimulationToolpathCutSummary` | 414 | one toolpath |
| `SimulationSemanticCutSummary` | 480 | one semantic item |
| `SimulationCutHotspot` | 509 | one hotspot window |
| `SimulationCutSummary` | 537 | the whole project |

Two accumulators build them, in the same file:

| Accumulator | Line | Builds |
|---|---|---|
| `SummaryAccumulator` | 1283 | the four scalar summaries |
| `KinematicsAccumulator` | 1314 | one `KinematicsSummary` |

`finalize_per_kinematics` (`simulation_cut.rs:1342`) folds the
accumulator array into a `BTreeMap<CutKinematics, KinematicsSummary>`.
The trace entry points are `SimulationCutTrace::from_samples`
(`simulation_cut.rs:1062`) and
`SimulationCutTrace::from_samples_with_context` (`:1096`).

A sixth rollup exists for drilling: `DrillToolpathSummary`
(`crates/rs_cam_core/src/ops/drill_metrics.rs`). The report did not
survey it. **(not determined)**

### 2.2 Which struct carries which `peak_*` field

This is the map a reader needs. The same field name appears on up to
four different structs, and they are not the same quantity.

| Field name | `KinematicsSummary` | `SimulationToolpathCutSummary` | `SimulationSemanticCutSummary` | `SimulationCutHotspot` | `SimulationCutSummary` |
|---|---|---|---|---|---|
| `peak_radial_woc_fraction` | 380 | — | — | — | — |
| `peak_axial_doc_fraction` (`Option`) | 391 | — | — | — | — |
| `peak_axial_doc_mm` | 393 | 440 | 498 | 525 | 559 |
| `peak_plunge_descent_mm` | 396 | 442 | 500 | 527 | 561 |
| `peak_chip_thickness_mm` (`Option`) | 406 | — | — | — | — |
| `peak_chipload_mm_per_tooth` | — | 439 | 497 | 524 | 558 |
| `peak_engagement` | — | — | 496 | — | — |
| `peak_mrr_mm3_s` | — | — | 503 | — | — |

Note two asymmetries a reader will trip on:

1. **`peak_engagement` is NOT on the per-toolpath summary.**
   `SimulationSemanticCutSummary::peak_engagement` (`:496`) exists.
   `SimulationToolpathCutSummary` has only `average_engagement` (`:438`).
   `SummaryAccumulator::peak_engagement` (`:1290`) computes it, and the
   toolpath summary does not publish it.
2. **`peak_chip_thickness_mm` exists on `KinematicsSummary` only**
   (`:406`). No scalar summary carries a chip-thickness extreme.

### 2.3 Which struct carries which mean or total

| Field name | Struct and line |
|---|---|
| `average_radial_woc_fraction` | `KinematicsSummary:378` |
| `average_axial_doc_fraction` (`Option`) | `KinematicsSummary:387` |
| `average_arc_radians` (`Option`) | `KinematicsSummary:400` |
| `average_mean_chip_thickness_mm` (`Option`) | `KinematicsSummary:404` |
| `average_leading_edge_speed_mm_min` | `KinematicsSummary:408` |
| `average_engagement` | `SimulationToolpathCutSummary:438`, `SimulationSemanticCutSummary:495`, `SimulationCutHotspot:523`, `SimulationCutSummary:557` |
| `average_mrr_mm3_s` | `SimulationToolpathCutSummary:444`, `SimulationSemanticCutSummary:502`, `SimulationCutHotspot:529`, `SimulationCutSummary:563` |
| `total_removed_volume_est_mm3` | `SimulationToolpathCutSummary:443`, `SimulationSemanticCutSummary:501`, `SimulationCutHotspot:528`, `SimulationCutSummary:562` |
| `air_cut_time_s` | `SimulationToolpathCutSummary:431`, `SimulationSemanticCutSummary:492`, `SimulationCutHotspot:520`, `SimulationCutSummary:550` |
| `low_engagement_time_s` | `SimulationToolpathCutSummary:434`, `SimulationSemanticCutSummary:493`, `SimulationCutHotspot:521`, `SimulationCutSummary:553` |
| `sample_count` | `KinematicsSummary:410`, `SimulationToolpathCutSummary:416`, `SimulationSemanticCutSummary:487`, `SimulationCutSummary:538` |
| `cutting_runtime_s` | `KinematicsSummary:375`, `SimulationToolpathCutSummary:418`, `SimulationSemanticCutSummary:490`, `SimulationCutHotspot:518`, `SimulationCutSummary:543` |

### 2.4 What survives, per metric

| Metric | Extreme kept | Mean kept | Anything else |
|---|---|---|---|
| Radial WOC fraction | yes, on `KinematicsSummary` | time-weighted | two threshold-time totals: `air_cut_time_s`, `low_engagement_time_s` |
| Axial DOC fraction | yes, `Option`, `KinematicsSummary` only | time-weighted, `Option` | none |
| Axial DOC mm | yes, on four structs | **no** | none |
| Plunge descent mm | yes, on four structs | **no** | none |
| Arc radians | **no** | time-weighted, `Option` | none |
| Mean chip thickness | **no** | time-weighted, `Option` | none |
| Peak chip thickness | yes, `Option`, `KinematicsSummary` only | **no** | none |
| Leading edge speed | **no** | time-weighted | none |
| Chipload per tooth | yes, on four structs | **no** | none |
| MRR | yes, `SimulationSemanticCutSummary` only | mean, four structs | none |
| Removed volume | — | total, four structs | none |
| **Spindle power** | **no summary of any kind exists** | — | — |
| **Tip deflection** | **no summary of any kind exists** | — | — |
| **Gantry push force** | **no quantity exists** | — | — |

The engine keeps an extreme, a time-weighted mean, or both. It keeps
`sample_count`. It keeps two threshold-time totals, and both trigger on
the radial-WOC axis only.

**The engine discards the shape of every metric.** No summary records a
second moment, a quantile, a bin count, or a time-above-limit for any of
the five limits in this design.

### 2.5 Is there any histogram, percentile or distribution in core?

**Almost none.** The report searched `crates/rs_cam_core/src` for
`histogram`, `percentile`, `quantile`, `p50`, `p95`, `p99`, `decile`,
`bucket` and `median`. Three results matter.

1. **`SeparationStats`**
   (`crates/rs_cam_core/src/finish/spiral_finish_compact.rs:193`).
   The only order-statistic summary type in core. Fields: `min`, `p50`,
   `p90`, `max`, `samples`. Its builder is `summarise`
   (`finish/spiral_finish_compact.rs:812`), which sorts in place and
   picks by index. It is **not** time-weighted. It does **not** derive
   from the cut trace. It measures pass separation in the compact
   spiral finisher.

2. **The chipload burn median.** `crate::tool_load::chipload::evaluate`
   (`chipload.rs:438`) sorts the per-sample chip thicknesses and takes
   the element at `len/2` (`chipload.rs:889-897`). One order statistic
   reaches the verdict, labelled `ChiploadStatistic::MedianLow`
   (`crates/rs_cam_core/src/tool_load/verdict.rs:813`). The sorted
   vector is discarded.

3. **`ChannelCounts`**
   (`crates/rs_cam_core/src/stock/sim_triage.rs:168`), built by
   `ChannelCounts::from_trace` (`sim_triage.rs:187`). Fields:
   `flagged_samples_air`, `flagged_samples_low`, `issue_segments`,
   `hotspots_total`, `samples_total`. It counts cutting samples in two
   radial-WOC bands, below 0.02 and 0.02 to 0.10. **This is the closest
   thing in core to "the share of the run past a limit."** It is a
   count, not a time share. It covers the engagement axis only. It
   covers none of the five limits.

Two related predicates reduce a population to a boolean and throw the
counts away:

- `crate::tool_load::chipload::is_bipolar_engagement`
  (`chipload.rs:265`). It counts samples below the band minimum and
  above the band maximum, compares each to `BIPOLAR_SIDE_FRACTION`
  (`chipload.rs:228`, 5 %), and returns `bool`.
- `CutterAxialConstraints::safe_band_is_empty`
  (`crates/rs_cam_core/src/feeds/cutter_constraints.rs:157`). It
  returns `Option<bool>`.

**The one true time-weighted share-past-a-threshold in core** is the
`AirCutRatios` trait (`simulation_cut.rs:620`), with
`AirCutRatios::air_cut_pct_of_total_runtime` (`:632`) and
`AirCutRatios::air_cut_pct_of_cutting_time` (`:652`). The trait doc is
worth reading in full: it exists because "air cut %" shipped under two
different denominators. `rebase_cutting_times` (`simulation_cut.rs:766`)
puts the three time fields on one clock.

**This is exactly the shape the new design needs. It exists for the
engagement axis only.**

---

## 3. Where each of the five limits is computed

### 3.1 Chip thickness floor

**The limit exists.** It comes from the embedded vendor LUT.

- Function: `crate::tool_load::chipload::matched_chip_envelope`
  (`crates/rs_cam_core/src/tool_load/chipload.rs:113`). It is
  `pub(crate)`. It builds a `LookupQuery` and calls
  `find_best_chip_envelope_row`.
- Inputs it reads: the tool geometry hint, `ToolDefinition::flute_count`,
  the material family, the hardness kind and value, the operation
  family, the pass role, and the engaged diameter.
- Refusal: `Material::Custom` returns `None` (`chipload.rs:121`).
- Routing: `crate::feeds::vendor_normalize::lut_query_for`
  (called at `chipload.rs:131`).
- The limit pair is `ChipBounds` (`verdict.rs:929`):
  `ChipBounds::min_mm_per_tooth: Option<f64>` (`:930`),
  `ChipBounds::max_mm_per_tooth: f64` (`:931`),
  `ChipBounds::source: ChipBoundsSource` (`:932`, enum at `:826`).
- **The floor is the `Option`.** A LUT row may publish only an upper
  bound.
- A DOC derate scales the band before use:
  `crate::feeds::geometry::derate_chipload_bounds`
  (called at `chipload.rs:581`).

The comparison lives on the type, never at the call site:

- `ChipBounds::exceeds_high` (`verdict.rs:943`)
- `ChipBounds::below_low` (`verdict.rs:951`) — returns `Option<bool>`
- `ChipBounds::contains` (`verdict.rs:959`)
- `ChipBounds::is_at_max` (`verdict.rs:974`)

All four delegate to `crate::tool_load::boundary`
(`crates/rs_cam_core/src/tool_load/boundary.rs`):
`boundary::exceeds_high` (`:105`), `boundary::below_low` (`:114`),
`boundary::BOUNDARY_EPSILON_REL` (`:86`).

### 3.2 Spindle power

**The limit exists.** It is machine-side and RPM-dependent.

- Function: `MachineProfile::power_at_rpm`
  (`crates/rs_cam_core/src/machine/mod.rs:302`).
- Inputs: `MachineProfile::power`, a `PowerModel` (`machine/mod.rs:26`),
  and the sample RPM. Two variants exist:
  `PowerModel::VfdConstantTorque { rated_power_kw, rated_rpm }` and
  `PowerModel::ConstantPower { power_kw }`.
- The gate multiplies by `MachineProfile::safety_factor`
  (`machine/mod.rs:105`) at the comparison site (`power.rs:439`).
- The gate publishes the limit as `PowerVerdict::Within::available_kw`
  and `PowerVerdict::Exceeds::available_kw` (`verdict.rs:1141`). The
  choice of which RPM's capacity to publish is at `power.rs:513-521`.

**The limit varies per sample.** A distribution of raw kilowatts cannot
be read against one number. A distribution of the ratio
`predicted / available` can.

### 3.3 Tool deflection

**The limit exists.** It is two fixed constants.

- `crate::tool_load::deflection::WITHIN_BOUND_MM = 0.050`
  (`crates/rs_cam_core/src/tool_load/deflection.rs:61`).
- `crate::tool_load::deflection::EXCEEDS_BOUND_MM = 0.200`
  (`deflection.rs:64`).
- Builder: `standard_bounds` (`deflection.rs:67`). It returns a
  `DeflectionBounds` (`verdict.rs:1233`) with fields
  `validated_within_mm` (`:1236`) and `exceeds_mm` (`:1240`).
- **Inputs: none.** The values are hard-coded. They do not vary with the
  tool, the material or the machine.

The band has three states, not two. Below 0.050 mm the verdict is
`Within` with `Confidence::Validated`. Between the two bounds the
verdict is `Within` with `Confidence::Approximate` and a
finish-degradation warning. Above 0.200 mm the verdict is `Exceeds`.

The planning-side inverse solves the same model for feed:

- `crate::feeds::force::chipload_cap_for_deflection_with_reason`
  (`crates/rs_cam_core/src/feeds/force.rs:225`)
- `crate::feeds::force::chipload_cap_for_deflection` (`force.rs:279`)
- `DeflectionCapRefusal` (`force.rs:168`) — the typed refusal:
  `NoLateralEngagement`, `EdgeForceOverBudget`, `Unmodelled`.

### 3.4 Depth of cut

**A limit exists, but it is a pre-simulation planning envelope, not a
post-simulation gate.**

There is no depth-of-cut criterion. `CriterionKind` (`verdict.rs:544`)
has six variants and none is depth of cut: `Chipload`, `Power`,
`Deflection`, `DrillChipWelding`, `DrillPeckAdequacy`,
`DrillPlungeFeed`.

The envelope:

- Function: `crate::feeds::cutter_constraints::cutter_axial_constraints`
  (`crates/rs_cam_core/src/feeds/cutter_constraints.rs:181`).
- Output: `CutterAxialConstraints` (`cutter_constraints.rs:100`).

Its four bounds and their sources:

| Field | Line | Source |
|---|---|---|
| `max_doc_deflection_mm` | 104 | `invert_deflection`, a binary search (called at `:198`) |
| `max_doc_vendor_mm: Option<f64>` | 109 | LUT row `ap_*_factor × diameter` or `ap_*_mm`, tighter wins (`max_doc_vendor`, called at `:206`) |
| `max_doc_scallop_mm: Option<f64>` | 114 | `max_doc_scallop` (called at `:211`); ball, bull and tapered-ball only, and only with a finish target |
| `min_doc_chipload_floor_mm: Option<f64>` | 119 | **the floor** — below it the arc-mean chip thickness drops under the LUT `chipload_min` |

Reducers and the binding label:

- `CutterAxialConstraints::safe_max_doc_mm` (`:142`) — minimum over the
  populated maxima.
- `CutterAxialConstraints::safe_band_is_empty` (`:157`) —
  `Option<bool>`; `None` means the floor was never modelled.
- `CutterAxialConstraints::binding_constraint` (`:125`) — an
  `AxialBindingConstraint` naming which bound won.

**Callers: `crate::feeds::suggest` only**
(`crates/rs_cam_core/src/feeds/suggest.rs:1920`, `:1951`, `:1963`,
`:1983`). The simulation path never calls it.

A second, unrelated axial limit caps the plunge **rate**, not the depth:
`crate::tool_load::plunge_stress::safe_plunge_cap_mm_min`
(`crates/rs_cam_core/src/tool_load/plunge_stress.rs:29`), from
`PLUNGE_CAP_PER_MM_TIP_DIAMETER = 150.0` (`plunge_stress.rs:20`). It
returns `None` for flat, bull and V-bit geometry. Its warning type is
`PlungeStressWarning` (`plunge_stress.rs:47`).

### 3.5 Gantry push force

**Confirmed: no model exists.** Four independent checks.

1. The module header of `crate::tool_load`
   (`crates/rs_cam_core/src/tool_load/mod.rs:1-16`) names **three**
   criteria and states they are all fully implemented: chipload, power,
   deflection. It also states there is no aggregate scalar "load %".
2. `ToolpathLoadVerdict` (`verdict.rs:221`) has exactly three gate
   fields — `chipload` (`:223`), `power` (`:224`), `deflection` (`:225`)
   — plus the optional `drill_gates` (`:234`). No force field exists.
3. `MachineProfile` (`machine/mod.rs:81`) has no force capacity field.
   Its fields are `name`, `spindle`, `power`, `chip_load`,
   `max_feed_mm_min`, `max_cutting_feed_mm_min`, `max_shank_mm`,
   `rigidity`, `safety_factor`, `kinematics`. `RigidityProfile`
   (`machine/mod.rs:55`) holds seven dimensionless DOC and WOC factors
   plus one WOC millimetre cap. None of them is a force.
4. A search for `gantry` in `crates/rs_cam_core/src` returns three hits.
   All three are comments stating that a feed number is a cutting
   ceiling and **not** the gantry travel rate
   (`feeds/mod.rs:2224`, `machine/mod.rs:127`,
   `tool_load/optimize/bounds.rs:228`). None computes a force.

**Core already computes the newtons a gantry-push model would need.**

- `crate::feeds::force::lateral_cutting_force`
  (`crates/rs_cam_core/src/feeds/force.rs:141`). It returns
  `Option<f64>` newtons. Inputs: the material, the axial DOC, the
  immersion angle, the feed per tooth. It returns `None` when any input
  is non-positive, or when the material carries no primary-source `Kc`.
- `crate::feeds::force::immersion_angle` (`force.rs:86`) — the arc from
  `ae/r`.
- `crate::feeds::force::affine_coefficients` (`force.rs:128`) — the
  `(Ks, F_edge)` pair.

Every caller is tool-side, never machine-side:

- `crate::feeds::predict` (`feeds/predict.rs:318`, `:433`) — the
  tip-deflection predictor.
- `crate::tool_load::deflection` module header (`deflection.rs:6`) — a
  doc reference.
- Its own tests (`force.rs:371-424`).

**(inferred)** The missing half is the machine-side capacity, not the
force model.

A planning document `THRUST_RESEARCH.md` sits in this same directory.
The report did not read it. The absence claim above is a claim about
**shipped core code only**.

---

## 4. The cost and storage picture for a distribution

### 4.1 How many samples a toolpath produces

The sample step is `request.resolution.max(0.25)` millimetres
(`crate::compute::simulate::run_simulation_memoized`,
`crates/rs_cam_core/src/compute/simulate.rs:867`), where `resolution` is
`SimulationRequest::resolution` (`compute/simulate.rs:215`).

Measured figures from the repository:

| Source | Samples |
|---|---|
| 2D perf golden, whole project | 17 112 (`crates/rs_cam_core/tests/fixtures/perf_golden_sim_metrics.json:4`, key `total_sample_count`) |
| 3D perf golden, whole project | 8 033 (`tests/fixtures/perf_golden_sim_metrics_3d.json:4`) |
| `bench_simulation_cut_trace_aggregation` arms | 50 000 and 250 000 (`crates/rs_cam_core/benches/perf_suite.rs:644`) |
| `bench_viz_triage_build` arms | 100 000 and 600 000 (`crates/rs_cam_core/benches/hot_paths.rs:1348`) |
| Census, one all-air toolpath | 20 328 cutting samples (`crates/rs_cam_core/src/stock/sim_triage.rs:23-25`) |
| `SummaryAccumulator::per_kinematics` design note | "aggregating 250 k samples is hot" (`simulation_cut.rs:1304-1307`) |

**Take 100 000 to 600 000 samples as the working range for a real
project.** The bench arms are the maintainers' own statement of what is
realistic.

### 4.2 How the trace is stored

- In memory: `SimulationCutTrace` (`simulation_cut.rs:907`). The samples
  are one flat `SimulationCutTrace::samples: Vec<SimulationCutSample>`
  (`:915`).
- The session holds `SimulationResult::cut_trace:
  Option<Arc<SimulationCutTrace>>` (`compute/simulate.rs:400`). The
  `Arc` makes clones cheap.
- **The trace is NOT in the project file.** `ProjectFile`
  (`crates/rs_cam_core/src/session/project_file.rs:25`) has four
  sections: `job`, `tools`, `models`, `setups`. A search for
  `cut_trace` and `SimulationCutTrace` in that file returns nothing.
- The trace **is** serialised to a standalone JSON artifact by
  `write_simulation_cut_artifact` (`simulation_cut.rs:1766`), which
  delegates to `crate::export::artifact_io::write_json_artifact`.
  `SIMULATION_CUT_TRACE_SCHEMA_VERSION` is currently 5
  (`simulation_cut.rs:21`).
- Capture is opt-in through `SimulationMetricOptions`
  (`simulation_cut.rs:13`): `enabled` and `capture_arc_engagement`.

Two fields are excluded from serialisation for a different reason.
`SimulationCutTrace::predicted_feeds` (`:981`) and
`SimulationCutTrace::modulated_feeds` (`:990`) carry `#[serde(skip)]`.
Their keys are tuples and JSON requires string keys. Both docs state the
maps are re-derivable.

### 4.3 The existing size and performance guards

Two guards exist. Both were written after a real failure.

1. **Disk.** `prune_simulation_cut_artifacts` (`simulation_cut.rs:1796`)
   with `PRUNE_GRACE_MS` (`simulation_cut.rs:1784`, ten minutes). Its
   doc records the incident: "at fine resolutions on a large board one
   dump is multiple GB, and an unbounded directory filled a 935 GB disk
   (96 GB / 81 dumps observed 2026-08-23)".

   **(inferred)** 96 GB over 81 dumps is about 1.2 GB per dump. At
   roughly 600 JSON bytes per sample that is about 2 million samples per
   dump. Treat this as an order of magnitude, not a measurement.

2. **Aggregation speed.** `SummaryAccumulator::per_kinematics`
   (`simulation_cut.rs:1308`) is a fixed-size array, not a `BTreeMap`.
   The comment above it states why: "aggregating 250 k samples is hot
   enough that map allocs + rebalances showed up as a 50% regression in
   the `simulation_cut_trace_aggregation/from_samples` bench"
   (`simulation_cut.rs:1304-1307`).

**No cap on the sample count exists.** The report searched
`simulation_cut.rs` for `MAX_SAMPLES`, `max_samples`, `sample_stride`,
`truncate` and `retain`. Nothing matched. The trace keeps every sample.

### 4.4 What a per-metric distribution would cost

**The accumulation itself is cheap.** `SummaryAccumulator` and
`KinematicsAccumulator` already walk every sample once and already do
the time-weighted arithmetic. A fixed-bin histogram adds one index
computation and one add per sample per metric — the same shape as the
existing `peak_*` maxima.

**Follow the array precedent, not the map precedent.** The 50 %
regression note on `SummaryAccumulator::per_kinematics` is the direct
warning. A fixed-size bin array costs one allocation per summary. A
`BTreeMap` of bins costs an allocation per distinct bin per summary.

**Storage is bounded.** A histogram is a fixed bin count, independent of
the sample count. **(inferred)** A `SimulationCutSample` is roughly 300
bytes of inline data plus the `span_path` heap vector, so 600 000
samples is roughly 180 MB in memory. A 32-bin histogram per metric per
toolpath is negligible beside that.

**Three things need care.**

1. **Power and deflection are not stored.** Sections 1.3 and 1.5. Each
   gate already makes a full-trace pass per toolpath — see the loops at
   `power.rs:372` and `deflection.rs:211`, and the filter in
   `chipload::steady_state_samples_for_toolpath` (`chipload.rs:301`),
   which scans the **whole** sample vector and filters by
   `toolpath_id`. The cost today is `O(toolpaths × samples)`. Adding a
   fourth full scan repeats that pattern. Adding the histogram inside
   the existing gate loop does not.

2. **The power limit varies per sample.** Section 3.2. Bin the ratio,
   not the raw kilowatts.

3. **The GUI rebuilds derived views every frame.** The `bench_viz_triage_build`
   comment states it: "The GUI rebuilds BOTH of these every frame from
   the full trace" (`benches/hot_paths.rs:1344-1345`). The two it names
   are `MeasurabilityReport::from_trace`
   (`crates/rs_cam_core/src/stock/sim_measurability.rs:304`) and the
   triage builder. **Compute a distribution once at trace-build time.**

**Quantiles cost more than bins.** Both existing order-statistic sites
sort: `summarise` (`finish/spiral_finish_compact.rs:812`) and the
chipload median (`chipload.rs:889`). A sort over 600 000 `f64` values
per metric per toolpath is a different cost class from a histogram. A
histogram gives approximate quantiles for free.

---

## 5. The closest existing pattern for a limit-plus-observation type

The codebase already has this pattern. Copy it. Do not invent one.
Every symbol below lives in
`crates/rs_cam_core/src/tool_load/verdict.rs` unless stated otherwise.

### 5.1 The direct match: `ChiploadMetric`

`ChiploadMetric` (`verdict.rs:993`) is the closest existing type. Four
fields:

- `ChiploadMetric::observed_mm_per_tooth: f64` (`:994`) — the value.
- `ChiploadMetric::statistic: ChiploadStatistic` (`:995`) — **which
  statistic produced the value.** The enum is at `verdict.rs:813`:
  `MedianLow`, `PeakHigh`, `PeakInRange`.
- `ChiploadMetric::evidence: SampleEvidence` (`:996`) — where it came
  from.
- `ChiploadMetric::bounds: ChipBounds` (`:997`) — the limit.

**The `statistic` field is the part worth copying most.** It names the
reduction, so a consumer never guesses whether a number is a peak or a
median.

### 5.2 How "the limit does not exist" is expressed

Two mechanisms, used together.

**Mechanism A: `Option` on the bound, and an `Option` accessor.**
`ChipBounds::min_mm_per_tooth: Option<f64>` (`verdict.rs:930`).
`ChipBounds::below_low` (`verdict.rs:951`) returns `Option<bool>`, not
`bool`. Its doc: `None` "is **not** the same as `Some(false)`, and the
`Option` is here so no caller can collapse 'unmodelled' into 'fine'."

**Mechanism B: a third verdict arm.** Every gate verdict has
`Unmodeled` beside `Within` and `Exceeds`:

- `ChiploadVerdict::Unmodeled` (enum at `verdict.rs:1005`)
- `PowerVerdict::Unmodeled` (enum at `verdict.rs:1141`)
- `DeflectionVerdict::Unmodeled` (enum at `verdict.rs:1247`)

The arm carries a typed `UnmodeledReason` (`verdict.rs:148`) with eleven
variants. Four matter to this design:

| Variant | Meaning here |
|---|---|
| `UnmodeledReason::NoVendorData` | the limit has no source |
| `UnmodeledReason::NotApplicableForOp(String)` | the limit has no meaning for this op |
| `UnmodeledReason::NotImplemented(String)` | the limit is deferred to a later phase |
| `UnmodeledReason::SimulationRequired` | no trace, or the trace misses this toolpath |

**`UnmodeledReason::NotImplemented` is the arm a gantry-push gate would
return on day one.**

`LoadState` (`verdict.rs:534`) is the three-valued projection:
`Within`, `Exceeds`, `Unmodeled`.

### 5.3 The cross-metric view: `CriterionStatus`

`CriterionStatus<'a>` (`verdict.rs:583`) is the uniform view across
gates. Fields: `kind`, `state`, `confidence`, `unmodeled_reason`,
`sample_range`, `population`, `display_peak`, `unit`, `exceeded`.

Each verdict produces one through its own method:

- `ChiploadVerdict::as_criterion_status` (`verdict.rs:1098`)
- `PowerVerdict::as_criterion_status` (`verdict.rs:1196`)
- `DeflectionVerdict::as_criterion_status` (`verdict.rs:1301`)

`CriterionKind` (`verdict.rs:544`) carries the labels:
`CriterionKind::label` (`verdict.rs:556`) and `CriterionKind::unit`
(`verdict.rs:567`). A new metric adds a variant to that enum.

The doc on `CriterionStatus::exceeded` states the consequence plainly:
including a gate in `ToolpathLoadVerdict::criteria` is the one decision
that makes it gate g-code export.

**This is the type a new surface should extend or mirror.**

### 5.4 The population contract: `GatePopulation`

`GatePopulation` (`verdict.rs:648`) is the X-VAC type. Fields:
`contributing` (`:651`), `offered` (`:654`), `unit` (`:657`, a
`PopulationUnit` at `verdict.rs:665`: `Samples` or `Holes`).

Methods: `GatePopulation::new` (`:680`),
`GatePopulation::is_vacuous` (`:690`),
`GatePopulation::filtered_out` (`:697`),
`GatePopulation::vacuity_clause` (`:704`).

The need it meets is precise. Its doc records that three gates once
returned `Within` with `sample_range 0..0` and `available_kw 0.0`, and
nothing said the verdict rested on nothing. It carries an explicit
tier rule: **the type is report-tier.** No gate outcome, threshold,
severity or export decision reads it.

`SampleEvidence` (`verdict.rs:723`) carries it as
`SampleEvidence::population: Option<GatePopulation>` (`:748`), with
`SampleEvidence::is_vacuous` (`:802`) and the builder
`SampleEvidence::with_population` (`:795`). The doc states `None` means
**not stated**, never "measured zero".

**A distribution needs this field.** A histogram over zero samples and a
histogram of a clean run are the same picture without it.

### 5.5 The pre-simulation precedent: `FeedsResult`

For the **before a simulation** stage, `FeedsResult`
(`crates/rs_cam_core/src/feeds/mod.rs:447`) already pairs a predicted
value with its limit:

- `FeedsResult::power_kw` (`feeds/mod.rs:455`) and
  `FeedsResult::available_power_kw` (`:456`), with the boolean
  `FeedsResult::power_limited` (`:457`).
- `FeedsResult::chipload_bounds: Option<ChiploadBounds>` (`:472`).
  `None` means the match supplied no band.

Two pre-simulation predictors exist:

- `crate::feeds::predict::predict_peak_deflection_um`
  (`crates/rs_cam_core/src/feeds/predict.rs:129`) — deflection.
  **Caveat: it returns `0.0` on any refusal, and callers treat zero as
  "no constraint signal". This is the silent-sentinel pattern the rest
  of the codebase has retired. Do not copy it.**
- `power_model_terms(...).kw_at_feed(feed)` (called at
  `feeds/mod.rs:2429`) — power, at the final feed.

### 5.6 The confidence axis

`Confidence` (`verdict.rs:205`): `Confidence::Validated` and
`Confidence::Approximate(String)`. The doc states `Validated` is rare,
and that the UI must render the two differently.

### 5.7 The comparison contract

`crate::tool_load::boundary`
(`crates/rs_cam_core/src/tool_load/boundary.rs`) owns the epsilon:
`boundary::BOUNDARY_EPSILON_REL` (`:86`), `boundary::exceeds_high`
(`:105`), `boundary::below_low` (`:114`). `ChipBounds` routes every
comparison through it.

The doc above `ChipBounds` (`verdict.rs:923-928`) records why: "a bare
comparison against `max_mm_per_tooth` is how G-CHIP-ULP shipped — a
verdict decided by the last bit of a multiply/divide round trip."

**A new limit type must put its comparison on the type, not at the call
site.**

### 5.8 The advisory precedent

Two fields show how core keeps a bound visible when a hard trip is not
defensible. Both are on `ChiploadVerdict::Within` (`verdict.rs:1005`):

- `burn_advisory: Option<Box<ChiploadMetric>>` — the median sat below
  the burn floor, but the floor's provenance is too weak to refuse on
  (`ChipBoundsSource::low_side_is_advisory`).
- `ceiling_advisory: Option<Box<ChiploadMetric>>` — the recipe sits on
  the band ceiling because the engine's own clamp put it there.

`EntrySpike` (`verdict.rs:896`) is the third: a sample that exceeded a
bound but was excluded from the gate trip. Fields: `observed`, `bound`,
`locality`, `side: Option<ChipSide>` (`ChipSide` at `verdict.rs:982`).
**`EntrySpike` is the smallest observed-plus-bound pair in the
codebase.**

### 5.9 The shape core has converged on

1. A **bound type**, with `Option` on any bound that may be absent, and
   the comparison as a method on the type. Model: `ChipBounds`.
2. A **metric type**, holding the observation, the **named statistic**,
   the evidence, and the bound type. Model: `ChiploadMetric`.
3. A **three-arm verdict enum**: `Within`, `Exceeds`,
   `Unmodeled(UnmodeledReason)`.
4. A **`GatePopulation`** on the evidence, so a vacuous pass is visible.
5. A uniform **`CriterionStatus`** projection for consumers.

---

## 6. Gaps

1. **The drill path is not covered.** `DrillToolpathSummary` and
   `DrillSample` (`crates/rs_cam_core/src/ops/drill_metrics.rs`) hang
   off `SimulationCutTrace::drill_samples` (`simulation_cut.rs:924`)
   and `SimulationCutTrace::drill_summaries` (`:932`). The drill gate
   trio (`crate::tool_load::drill_gates::DrillGatesVerdict`) is a fourth
   gate family with its own population unit,
   `PopulationUnit::Holes`. The report did not survey them. A
   distribution design covering the whole project must decide what a
   drill toolpath contributes. **(not determined)**

2. **The per-sample memory size is an estimate.** Section 4.4 gives
   roughly 300 bytes. The report added field sizes by hand. It did not
   evaluate `size_of::<SimulationCutSample>()`, because the task forbids
   running cargo. **(inferred)**

3. **The artifact size per sample is an estimate.** Section 4.3 derives
   about 600 JSON bytes per sample from the 96 GB / 81 dumps figure in
   the `prune_simulation_cut_artifacts` doc. The report did not measure
   a real artifact. **(inferred)**

4. **The GUI side is out of scope by instruction.** Another agent covers
   it. The one GUI fact this report relies on is the comment in
   `bench_viz_triage_build` (`benches/hot_paths.rs:1344-1345`), which
   cites `ui/sim_diagnostics.rs:588-589`. The report did not verify that
   citation. **(not determined)**

5. **`THRUST_RESEARCH.md` was not read.** It sits in this same planning
   directory and may already hold the gantry-push model section 3.5
   reports as absent from core. The absence claim covers **shipped core
   code only**.

6. **The default value of `SimulationRequest::resolution` is not
   known.** Core clamps it with `.max(0.25)`
   (`compute/simulate.rs:867`). The report did not find where the field
   gets its default, because that lives outside core. **(not
   determined)**

7. **No time-weighted order statistic exists anywhere in the crate.**
   Both sorting sites — `summarise`
   (`finish/spiral_finish_compact.rs:812`) and the chipload median
   (`chipload.rs:889`) — sort raw values and ignore `segment_time_s`. A
   time-weighted median or percentile would be new machinery. The
   report verified this absence by searching `crates/rs_cam_core/src`
   for `percentile`, `quantile` and `median`.
