# SURVEY_CORE — the load model data path in `rs_cam_core`

Read-only survey. Date: 2026-09-17. Scope: `crates/rs_cam_core` only.
Another session moves files in this tree. Every path below comes from a
live `rg` or `sed` read on 2026-09-17, not from memory or a doc comment.

Each claim carries a `file:line` citation. The report marks an inference
with **(inferred)**. The report marks a gap with **(not determined)**.

---

## 1. Per-sample data available, per metric

The per-sample record is `SimulationCutSample`
(`crates/rs_cam_core/src/stock/simulation_cut.rs:163`). The engine emits
one sample per subsegment of a move. The simulator cuts a move into
`max(⌈len/sample_step⌉, ⌈|Δz|/0.02⌉)` subsegments
(`crates/rs_cam_core/src/dexel_stock/simulation.rs:1591-1609`).

### 1.1 The time weight

Two time fields exist on every sample:

- `cumulative_time_s` — the clock at this sample
  (`simulation_cut.rs:168`).
- `segment_time_s` — the duration this sample represents
  (`simulation_cut.rs:169`).

`segment_time_s` is the time weight. The existing accumulators already
multiply by it. See `KinematicsAccumulator::radial_woc_time_weighted_sum`
and its siblings (`simulation_cut.rs:1315-1337`). A time-weighted
distribution is therefore possible from the stored data.

### 1.2 Chip thickness

Four distinct quantities exist. Do not mix them.

| Quantity | Field | Line | Optional |
|---|---|---|---|
| Commanded advance per tooth | `chipload_mm_per_tooth` | `simulation_cut.rs:191` | no |
| Arc-mean chip thickness (gate input) | `effective_chip_thickness_mm` | `simulation_cut.rs:193` | yes |
| Arc-mean chip thickness (engagement vector) | `engagement.mean_chip_thickness_mm` | `simulation_cut.rs:103` | yes |
| Arc-peak chip thickness | `engagement.peak_chip_thickness_mm` | `simulation_cut.rs:115` | yes |

The doc comment at `simulation_cut.rs:95-108` states the two differ by
`(2/arc)·(1 − cos(arc/2))·sin(arc)` below full slotting. The factor is
about 0.373 at half immersion. The chipload gate reads
`effective_chip_thickness_mm` (`tool_load/chipload.rs:772`; see also the module header at `tool_load/chipload.rs:22`).

**Enough for a distribution: yes.** The value is per sample. The sample
carries `segment_time_s`.

### 1.3 Spindle power

**No power value exists on the sample.** The report read every field of
`SimulationCutSample` (`simulation_cut.rs:163-242`). No field names
power, torque or watts.

The power gate computes power per sample at gate time. See the sample
loop at `tool_load/power.rs:372` and the call to `predicted_power_kw` at
`tool_load/power.rs:418`. The model inputs are `PowerModelInputs`
(`tool_load/power.rs:107-135`): `kc_n_per_mm2`, `cross_section_mm2`,
`axial_doc_mm`, `immersion_rad`, `engagement_diameter_mm`, `spindle_rpm`
and `flute_count`. The gate reads all of these from the sample, the
tool and the material.

**Enough for a distribution: yes, but only after re-computation.** The
inputs are per sample. A distribution pass must call
`predicted_power_kw` again, or the emitter must store the result.

### 1.4 Axial depth of cut

Three separate axial fields exist. The engine split them deliberately.

- `axial_doc_mm` (`simulation_cut.rs:178`) — the legacy wire name. A
  pure-vertical plunge reports `0.0` here.
- `axial_engagement_mm` (`simulation_cut.rs:182`) — the lateral, arc and
  helix axial engagement. The deflection gate and the chip-geometry
  gates read this axis.
- `plunge_descent_mm` (`simulation_cut.rs:187`) — the Z descent of a
  pure-vertical plunge sample.

`engagement.axial_doc_fraction` (`simulation_cut.rs:84`) holds the same
quantity as a fraction of flute length. It is `Option<f64>`. `None`
means **not measured**. `Some(0.0)` means measured and zero.

**Enough for a distribution: yes.** All three are per sample.

### 1.5 Deflection

**No deflection value exists on the sample.** The gate computes tip
deflection per sample through `sample_tip_deflection_mm`
(`tool_load/deflection.rs:80-113`). That function reads
`sample.engagement.radial_woc_fraction`, `sample.axial_engagement_mm`
and `sample.arc_engagement_radians`. It then calls
`feeds::predict::tip_deflection_from_engagement`
(`tool_load/deflection.rs:107`).

**Enough for a distribution: yes, but only after re-computation.** The
same condition as power applies.

### 1.6 Gantry push force

**No force value exists on the sample.** No force value exists in the
trace at all. See section 3.5.

### 1.7 Other fields a distribution needs

- `is_cutting` (`simulation_cut.rs:170`) — the in-cut predicate.
- `in_transit_span` (`simulation_cut.rs:217`) — the transit flag. The
  existing peak metrics skip transit samples to avoid lift-bridge
  artifacts (`simulation_cut.rs:206-216`).
- `cut_kinematics` (`simulation_cut.rs:172`) — Linear, Plunge, Helix,
  Arc or Rapid (`simulation_cut.rs:27-32`).
- `toolpath_id` (`simulation_cut.rs:164`) and `move_index`
  (`simulation_cut.rs:165`).
- `span_path` (`simulation_cut.rs:204`) and `source_intent`
  (`simulation_cut.rs:242`).

---

## 2. What the current summaries keep, and what they discard

### 2.1 The summary structs

Five summary structs exist in `stock/simulation_cut.rs`:

| Struct | Line | Scope |
|---|---|---|
| `KinematicsSummary` | 373 | one `CutKinematics` class |
| `SimulationToolpathCutSummary` | 414 | one toolpath |
| `SimulationSemanticCutSummary` | 480 | one semantic item |
| `SimulationCutHotspot` | 509 | one hotspot window |
| `SimulationCutSummary` | 537 | the project |

Two accumulators build them: `SummaryAccumulator`
(`simulation_cut.rs:1283`) and `KinematicsAccumulator`
(`simulation_cut.rs:1313`).

A sixth rollup exists for drilling: `DrillToolpathSummary`
(`ops/drill_metrics.rs`, referenced at `simulation_cut.rs:2`). The
report did not examine it in detail **(not determined)**.

### 2.2 What survives per metric

| Metric | Peak kept | Mean kept | Anything else |
|---|---|---|---|
| Radial WOC fraction | yes (`simulation_cut.rs:380`) | time-weighted (`:378`) | time below 0.02 and time in 0.02..0.10 (`:432`, `:435`) |
| Axial DOC fraction | yes, `Option` (`:391`) | time-weighted, `Option` (`:387`) | none |
| Axial DOC mm | yes (`:393`) | **no** | none |
| Plunge descent mm | yes (`:396`) | **no** | none |
| Arc radians | **no** | time-weighted, `Option` (`:399`) | none |
| Mean chip thickness | **no** | time-weighted, `Option` (`:402`) | none |
| Peak chip thickness | yes, `Option` (`:406`) | **no** | none |
| Leading edge speed | **no** | time-weighted (`:408`) | none |
| Chipload per tooth | yes (`:439`) | **no** | none |
| MRR | yes (`:503`) | mean (`:502`) | none |
| Removed volume | — | total (`:501`) | none |
| Spindle power | **not summarised at all** | — | — |
| Tip deflection | **not summarised at all** | — | — |

The engine keeps the extreme, the time-weighted mean, or both. It keeps
the sample count (`simulation_cut.rs:410`, `:416`, `:538`). It keeps two threshold-time
totals on the radial-WOC axis only.

**The engine discards the shape of every metric.** No summary records a
second moment, a quantile, a bin count, or a time above a limit for any
of the five limits in this design.

### 2.3 Is there any histogram, percentile or distribution in core?

**Almost none.** The report searched the whole crate for `histogram`,
`percentile`, `quantile`, `p50`, `p95`, `p99`, `decile` and `bucket`.
Three results matter:

1. **`SeparationStats`** (`finish/spiral_finish_compact.rs:191-204`).
   This is the only order-statistic summary type in core. It holds
   `min`, `p50`, `p90`, `max` and `samples`. The helper `summarise`
   (`finish/spiral_finish_compact.rs:811-830`) sorts in place and picks
   by index. It is **not** time-weighted. It does **not** derive from
   the cut trace. It measures pass separation in the compact spiral
   finisher.

2. **The chipload burn median** (`tool_load/chipload.rs:885-897`). The
   gate sorts the per-sample chip thicknesses and takes the element at
   `len/2`. This is one order statistic, computed and then discarded.
   Only the single median value reaches the verdict, through
   `ChiploadStatistic::MedianLow` (`tool_load/verdict.rs:816`).

3. **`ChannelCounts`** (`stock/sim_triage.rs:168-212`). It counts
   cutting samples in two radial-WOC bands: below 0.02, and 0.02 to
   0.10. This is the closest thing in core to "the share of the run past
   a limit". It is a **count**, not a time share. It covers the
   engagement axis only. It covers none of the five limits.

`is_bipolar_engagement` (`tool_load/chipload.rs:265-293`) counts samples
below the band minimum and samples above the band maximum. It compares
both counts to 5 % of the valid samples. It returns a boolean. It
discards the two counts.

`air_cut_time_s` (`simulation_cut.rs:432`) is a time-weighted share past
a threshold. The `AirCutRatios` trait (`simulation_cut.rs:600-660`)
converts it to a percentage. The trait doc names the two denominators
and warns that both have shipped as "air cut %". **This is the shape the
new design needs, but it exists for the engagement axis only.**

---

## 3. Where each of the five limits is computed

### 3.1 Chip thickness floor

**The limit exists.** It comes from the embedded vendor LUT.

- Function: `matched_chip_envelope` (`tool_load/chipload.rs:113-150`).
  It builds a `LookupQuery` and calls `find_best_chip_envelope_row`.
- Inputs: the tool geometry hint, the flute count, the material family,
  the hardness kind and value, the operation family, the pass role, and
  the engaged diameter.
- Refusal: `Material::Custom` returns `None`
  (`tool_load/chipload.rs:121`).
- The limit pair lands in `ChipBounds` (`tool_load/verdict.rs:929-933`):
  `min_mm_per_tooth: Option<f64>`, `max_mm_per_tooth: f64`,
  `source: ChipBoundsSource`.
- The floor is `min_mm_per_tooth`. It is an `Option`. A LUT row may ship
  only an upper bound. The doc at `tool_load/verdict.rs:916-919` states
  this directly.
- A DOC derate scales the bounds before use
  (`tool_load/chipload.rs:566-580`). The derate calls
  `geometry::derate_chipload_bounds`.

The gate compares through the type, never with a bare `>`. See
`ChipBounds::exceeds_high`, `ChipBounds::below_low` and
`ChipBounds::contains` (`tool_load/verdict.rs:939-963`).
`below_low` returns `Option<bool>`. `None` means the row publishes no
minimum. The doc states that `None` is not the same as `Some(false)`
(`tool_load/verdict.rs:951-953`).

### 3.2 Spindle power

**The limit exists.** It is machine-side and RPM-dependent.

- Function: `MachineProfile::power_at_rpm` (`machine/mod.rs:302-315`).
- Inputs: the `PowerModel` enum and the sample RPM. Two variants exist:
  `VfdConstantTorque { rated_power_kw, rated_rpm }` and
  `ConstantPower { power_kw }`.
- The gate multiplies by `MachineProfile::safety_factor`
  (`machine/mod.rs:105`) at the comparison site
  (`tool_load/power.rs:439`).
- The gate publishes the limit as `available_kw` on both the `Within`
  and the `Exceeds` arms (`tool_load/verdict.rs:1143`, `:1155`). The
  selection of which RPM's capacity to publish is at
  `tool_load/power.rs:513-521`.

The limit is therefore **per sample**, not per toolpath. A distribution
must compare each sample against its own `available_kw`, or must
normalise to a ratio first.

### 3.3 Tool deflection

**The limit exists.** It is two fixed constants.

- Constants: `WITHIN_BOUND_MM = 0.050` (`tool_load/deflection.rs:61`)
  and `EXCEEDS_BOUND_MM = 0.200` (`tool_load/deflection.rs:64`).
- Function: `standard_bounds` (`tool_load/deflection.rs:67-72`). It
  returns a `DeflectionBounds` (`tool_load/verdict.rs:1233-1242`).
- Inputs: none. The values are hard-coded. They do not vary with the
  tool, the material or the machine.

The band has three states, not two. Below 0.050 mm the verdict is
`Within(Validated)`. Between the two the verdict is
`Within(Approximate)` with a finish-degradation warning. Above 0.200 mm
the verdict is `Exceeds` (`tool_load/verdict.rs:1231-1241`).

A separate inverse exists for planning. `chipload_cap_for_deflection`
(`feeds/force.rs:279`) and `chipload_cap_for_deflection_with_reason`
(`feeds/force.rs:225`) solve the affine force model for the feed that
holds deflection at a given bound.

### 3.4 Depth of cut

**A limit exists, but it is a pre-simulation planning envelope, not a
post-simulation gate.** There is no `CriterionKind::Doc`. The criterion
enum has six variants and none is depth of cut
(`tool_load/verdict.rs:544-551`): `Chipload`, `Power`, `Deflection`,
`DrillChipWelding`, `DrillPeckAdequacy`, `DrillPlungeFeed`.

- Function: `cutter_axial_constraints`
  (`feeds/cutter_constraints.rs:181`).
- Output: `CutterAxialConstraints` (`feeds/cutter_constraints.rs:100`).
- The envelope holds four bounds:
  - `max_doc_deflection_mm` (`:104`) — from `invert_deflection`
    (`feeds/cutter_constraints.rs:198`), a binary search.
  - `max_doc_vendor_mm: Option<f64>` (`:109`) — from the LUT row's
    `ap_*_factor × diameter` or `ap_*_mm`, tighter wins.
  - `max_doc_scallop_mm: Option<f64>` (`:114`) — ball, bull and
    tapered-ball only, and only when the caller supplies a finish
    target.
  - `min_doc_chipload_floor_mm: Option<f64>` (`:119`) — the **floor**,
    below which the arc-mean chip thickness collapses under the LUT
    `chipload_min`.
- Reducers: `safe_max_doc_mm` (`:142`) takes the minimum of the
  populated maxima. `safe_band_is_empty` (`:157`) returns
  `Option<bool>`; `None` means the floor was never modelled.
- `binding_constraint` (`:120`) names which bound produced the safe
  maximum.

Callers: `feeds::suggest` only (`feeds/suggest.rs:1920`, `:1951`,
`:1963`, `:1983`). The simulation path does not call it.

A second, unrelated axial limit exists for plunging:
`safe_plunge_cap_mm_min` (`tool_load/plunge_stress.rs:30-41`). It caps
the plunge **rate**, not the depth. It applies to ball and tapered-ball
geometry only.

### 3.5 Gantry push force

**Confirmed: no model exists.** The report states this with four
independent checks.

1. The module header of `tool_load/mod.rs:1-16` names **three**
   criteria and states they are all fully implemented: chipload, power,
   deflection. It also states there is no aggregate scalar "load %".
2. `ToolpathLoadVerdict` (`tool_load/verdict.rs:221-273`) has three
   gate fields plus the optional drill trio. No force field exists.
3. `MachineProfile` (`machine/mod.rs:81-115`) has no force capacity
   field. Its fields are `name`, `spindle`, `power`, `chip_load`,
   `max_feed_mm_min`, `max_cutting_feed_mm_min`, `max_shank_mm`,
   `rigidity`, `safety_factor` and `kinematics`. `RigidityProfile`
   (`machine/mod.rs:55-63`) holds only dimensionless DOC and WOC
   factors, and one WOC millimetre cap. None of them is a force.
4. A search for `gantry` across `crates/rs_cam_core/src` returns three
   hits. All three are comments that say a feed number is a cutting
   ceiling and **not** the gantry travel rate
   (`feeds/mod.rs:2221`, `machine/mod.rs:127`,
   `tool_load/optimize/bounds.rs:228`). None computes a force.

**A force value does exist, but nothing compares it to a machine
limit.** `lateral_cutting_force` (`feeds/force.rs:141-158`) returns the
lateral cutting force in newtons. Its inputs are the material, the axial
DOC, the immersion angle and the feed per tooth. It returns `None` when
any input is non-positive, or when the material carries no
primary-source `Kc`.

Its callers are all tool-side, never machine-side:
- `feeds/predict.rs:318` and `feeds/predict.rs:433` — the tip-deflection
  predictor.
- `feeds/force.rs:371-424` — its own tests.
- `tool_load/deflection.rs:6` — a doc reference.

So core already computes the newton figure a gantry-push model would
need. Core has nowhere to compare it. **(inferred: the missing half is
the machine-side capacity, not the force model.)**

A planning document `THRUST_RESEARCH.md` exists in this directory. The
report did not read it, because the task scope is core code.

---

## 4. The cost and storage picture for a distribution

### 4.1 How many samples a toolpath produces

The sample step is `request.resolution.max(0.25)` millimetres
(`compute/simulate.rs:867`). A move produces
`max(⌈len/step⌉, ⌈|Δz|/0.02⌉)` samples
(`dexel_stock/simulation.rs:1591`).

Measured figures from the repository:

| Source | Samples |
|---|---|
| 2D perf golden, whole project | 17 112 (`tests/fixtures/perf_golden_sim_metrics.json:4`) |
| 3D perf golden, whole project | 8 033 (`tests/fixtures/perf_golden_sim_metrics_3d.json:4`) |
| Aggregation bench arms | 50 000 and 250 000 (`benches/perf_suite.rs:644`) |
| Triage bench arms | 100 000 and 600 000 (`benches/hot_paths.rs:1348`) |
| Census, one all-air toolpath | 20 328 cutting samples (`stock/sim_triage.rs:23-25`) |
| Accumulator design note | "aggregating 250 k samples is hot" (`stock/simulation_cut.rs:1306`) |

**Take 100 000 to 600 000 samples as the working range for a real
project.** The bench arms are the maintainers' own statement of what is
realistic.

### 4.2 How the trace is stored

- In memory: `SimulationCutTrace` (`stock/simulation_cut.rs:907`). The
  samples live in one flat `Vec<SimulationCutSample>`
  (`stock/simulation_cut.rs:915`).
- The session holds it as `Option<Arc<SimulationCutTrace>>` on
  `SimulationResult` (`compute/simulate.rs:400`). The `Arc` means clones
  are cheap.
- **The trace is NOT in the project file.** `ProjectFile`
  (`session/project_file.rs:25-36`) has four sections: `job`, `tools`,
  `models`, `setups`. No trace field exists. A search for `cut_trace`
  and `SimulationCutTrace` in `session/project_file.rs` returns nothing.
- The trace **is** serialised to a standalone JSON artifact.
  `write_simulation_cut_artifact` (`stock/simulation_cut.rs:1766-1776`)
  writes it. `SIMULATION_CUT_TRACE_SCHEMA_VERSION` is currently 5
  (`stock/simulation_cut.rs:20`).

### 4.3 The existing size guards

Two guards exist, and both were written after a real failure.

1. **Disk.** `prune_simulation_cut_artifacts`
   (`stock/simulation_cut.rs:1796-1832`). Its doc records the incident:
   "at fine resolutions on a large board one dump is multiple GB, and an
   unbounded directory filled a 935 GB disk (96 GB / 81 dumps observed
   2026-08-23)". The function keeps the newest `keep` files. It never
   prunes a file younger than 10 minutes (`PRUNE_GRACE_MS`,
   `stock/simulation_cut.rs:1783`).

   **(inferred)** 96 GB over 81 dumps is about 1.2 GB per dump. At
   roughly 600 JSON bytes per sample that is about 2 million samples per
   dump. Treat this as an order of magnitude, not a measurement.

2. **Aggregation speed.** The per-kinematics accumulator uses a
   fixed-size array, not a `BTreeMap`. The comment states why:
   "aggregating 250 k samples is hot enough that map allocs + rebalances
   showed up as a 50% regression in the
   `simulation_cut_trace_aggregation/from_samples` bench"
   (`stock/simulation_cut.rs:1304-1307`).

Two fields are excluded from serialisation for a different reason.
`predicted_feeds` (`stock/simulation_cut.rs:980`) and `modulated_feeds`
(`stock/simulation_cut.rs:989`) carry `#[serde(skip)]`. Their keys are
tuples, and JSON requires string keys. Both are re-derivable.

**No cap on the sample count exists.** The report searched
`stock/simulation_cut.rs` for `MAX_SAMPLES`, `max_samples`,
`sample_stride`, `truncate` and `retain`. Nothing matched. The trace
keeps every sample.

### 4.4 What a per-metric distribution would cost

**The accumulation itself is cheap.**

The engine already walks every sample once to build the summaries.
`SummaryAccumulator` (`stock/simulation_cut.rs:1283`) and
`KinematicsAccumulator` (`stock/simulation_cut.rs:1313`) already do the
time-weighted arithmetic. A fixed-bin histogram adds one index
computation and one add per sample per metric. That is the same shape as
the existing `peak_*` maxima.

**Follow the array precedent, not the map precedent.** The 50 %
regression note at `stock/simulation_cut.rs:1304` is the direct warning.
A fixed-size bin array costs one allocation per summary. A `BTreeMap`
of bins costs an allocation per distinct bin per summary.

**The storage cost is real but bounded.** A histogram is a fixed number
of bins, independent of the sample count. Compare this with the trace
itself: **(inferred)** a `SimulationCutSample` is roughly 300 bytes of
stack data plus the `span_path` heap vector, so 600 000 samples is
roughly 180 MB in memory. A 32-bin histogram per metric per toolpath is
negligible beside that.

**Three things need care.**

1. **Power and deflection are not stored.** Section 1.3 and 1.5. A
   distribution for either needs a re-computation pass. Each gate
   already makes that pass per toolpath
   (`tool_load/power.rs:372`, `tool_load/deflection.rs:211`). Each pass
   scans the **whole** sample vector and filters by `toolpath_id`
   (`tool_load/chipload.rs:301-326`). The cost today is therefore
   `O(toolpaths × samples)`. Adding a fourth full scan repeats that
   pattern. Adding the histogram to the existing gate loop does not.

2. **The power limit varies per sample.** Section 3.2. A histogram of
   raw kilowatts cannot be read against one number. A histogram of the
   ratio `predicted / available` can.

3. **The GUI rebuilds derived views every frame.** The bench comment
   states it: "The GUI rebuilds BOTH of these every frame from the full
   trace" (`benches/hot_paths.rs:1344-1345`). Compute a distribution
   once at trace-build time. Do not compute it per frame.

**Quantiles are more expensive than bins.** The existing `summarise`
helper sorts in place (`finish/spiral_finish_compact.rs:813`). The
chipload median also sorts (`tool_load/chipload.rs:889`). A sort over
600 000 f64 values per metric per toolpath is a different cost class
from a histogram. A histogram gives approximate quantiles for free.

---

## 5. The closest existing pattern for a limit-plus-observation type

The codebase already has this pattern. Copy it. Do not invent one.

### 5.1 The direct match: `ChiploadMetric`

`ChiploadMetric` (`tool_load/verdict.rs:993-1001`) is the closest
existing type. It has four fields:

- `observed_mm_per_tooth: f64` — the value.
- `statistic: ChiploadStatistic` — **which statistic produced the
  value** (`tool_load/verdict.rs:813-822`): `MedianLow`, `PeakHigh` or
  `PeakInRange`.
- `evidence: SampleEvidence` — where the value came from.
- `bounds: ChipBounds` — the limit.

The `statistic` field is the part worth copying most. It names the
reduction, so a consumer never guesses whether a number is a peak or a
median.

### 5.2 How "the limit does not exist" is expressed

Two mechanisms carry this, and they are used together.

**Mechanism A: `Option` on the bound.** `ChipBounds::min_mm_per_tooth`
is `Option<f64>` (`tool_load/verdict.rs:930`). The accessor
`ChipBounds::below_low` returns `Option<bool>`, not `bool`
(`tool_load/verdict.rs:955-960`). The doc is explicit: `None` "is **not**
the same as `Some(false)`, and the `Option` is here so no caller can
collapse 'unmodelled' into 'fine'."

**Mechanism B: a third verdict arm.** Every gate verdict has an
`Unmodeled` arm beside `Within` and `Exceeds`:

- `ChiploadVerdict::Unmodeled` (`tool_load/verdict.rs:1058`)
- `PowerVerdict::Unmodeled` (`tool_load/verdict.rs:1159`)
- `DeflectionVerdict::Unmodeled` (`tool_load/verdict.rs:1265`)

The arm carries a typed `UnmodeledReason`
(`tool_load/verdict.rs:148-196`) with eleven variants. Three of them
matter to this design:

- `NoVendorData` — the limit has no source.
- `NotApplicableForOp(String)` — the limit has no meaning here.
- `NotImplemented(String)` — the limit is deferred to a later phase.
  **This is the arm a gantry-push gate would use on day one.**

`LoadState` (`tool_load/verdict.rs:534-540`) is the three-valued
projection: `Within`, `Exceeds`, `Unmodeled`.

### 5.3 The cross-metric view: `CriterionStatus`

`CriterionStatus` (`tool_load/verdict.rs:583-604`) is the uniform view
across gates. Each verdict produces one through its own
`as_criterion_status` method (`tool_load/verdict.rs:1099`,
`:1195`, `:1300`). It holds `kind`, `state`, `confidence`,
`unmodeled_reason`, `sample_range`, `population`, `display_peak`, `unit`
and `exceeded`.

`CriterionKind::unit` (`tool_load/verdict.rs:573-582`) gives the unit
string per metric. A new metric adds a variant there.

**This is the type a new surface should extend or mirror.** It already
answers "what kind, what state, what peak, what range" without exposing
gate internals.

### 5.4 The population contract: `GatePopulation`

`GatePopulation` (`tool_load/verdict.rs:648-661`) is the X-VAC type. It
holds `contributing`, `offered` and `unit`
(`PopulationUnit::Samples` or `::Holes`).

The design need this meets is precise. Three gates once returned
`Within` with `sample_range 0..0` and `available_kw 0.0`. Nothing said
the verdict rested on nothing (`tool_load/verdict.rs:627-640`).
`GatePopulation::is_vacuous` (`tool_load/verdict.rs:685`) is the
predicate. `vacuity_clause` (`tool_load/verdict.rs:698-711`) is the one
shared operator-facing sentence.

**A distribution needs this field.** A histogram over zero samples and a
histogram showing a clean run are the same picture without it.

### 5.5 The pre-simulation precedent: `FeedsResult`

For the **before a simulation** stage, `FeedsResult`
(`feeds/mod.rs:447-...`) already pairs a predicted value with its limit:

- `power_kw` (`feeds/mod.rs:455`) and `available_power_kw`
  (`feeds/mod.rs:456`), with the boolean `power_limited`
  (`feeds/mod.rs:457`).
- `chipload_bounds: Option<ChiploadBounds>` (`feeds/mod.rs:465`).
  `None` means the match supplied no band.

Two pre-simulation predictors exist:

- `predict_peak_deflection_um` (`feeds/predict.rs:129`) — deflection.
  Note the caveat: it returns `0.0` on any refusal, and callers treat
  zero as "no constraint signal". **This is the silent-sentinel pattern
  the rest of the codebase has retired.** Do not copy it.
- `power_model_terms(...).kw_at_feed(feed)` (`feeds/mod.rs:2429`) —
  power, at the final feed.

### 5.6 The confidence axis

`Confidence` (`tool_load/verdict.rs:205-213`) has two variants:
`Validated` and `Approximate(String)`. The doc states that `Validated`
is rare, and that the UI must render the two differently.

### 5.7 The comparison contract

`tool_load/boundary` holds the comparison epsilon. `ChipBounds` routes
every comparison through it (`tool_load/verdict.rs:941`, `:958`,
`:975`, `:986`). The doc at `tool_load/verdict.rs:923-928` records why:
"a bare comparison against `max_mm_per_tooth` is how G-CHIP-ULP shipped
— a verdict decided by the last bit of a multiply/divide round trip."

**A new limit type must put its comparison on the type, not at the call
site.**

### 5.8 Recommended shape, as a summary of what exists

The pattern the codebase has converged on is:

1. A bound type, with `Option` on any bound that may be absent, and the
   comparison as a method on the type.
2. A metric type, holding the observation, the **named statistic**, the
   evidence, and the bound type.
3. A three-arm verdict enum: `Within`, `Exceeds`, `Unmodeled(reason)`.
4. A `GatePopulation` on the evidence, so a vacuous pass is visible.
5. A uniform `CriterionStatus` projection for consumers.

---

## 6. Gaps

These are the questions this survey could not answer, or answered only
in part.

1. **The drill summary is not covered.** `DrillToolpathSummary` and
   `DrillSample` live in `ops/drill_metrics.rs`. The trace carries both
   (`stock/simulation_cut.rs:924`, `:932`). The drill gate trio
   (`tool_load/drill_gates.rs`) is a fourth gate family with its own
   population unit (`PopulationUnit::Holes`). The report did not survey
   them. A distribution design that claims to cover the whole project
   must decide what a drill toolpath contributes. **(not determined)**

2. **The per-sample memory size is an estimate.** Section 4.4 gives
   roughly 300 bytes per sample. The report derived this by adding field
   sizes by hand. It did not measure `size_of::<SimulationCutSample>()`,
   because the task forbids running cargo. **(inferred)**

3. **The artifact size per sample is an estimate.** Section 4.3 derives
   about 600 JSON bytes per sample from the 96 GB / 81 dumps figure. The
   report did not measure a real artifact. **(inferred)**

4. **The GUI side is out of scope by instruction.** Another agent covers
   it. The one GUI fact this report relies on is the bench comment at
   `benches/hot_paths.rs:1344-1345`, which states that the GUI rebuilds
   the measurability report and the triage view every frame from the
   full trace. That comment cites `ui/sim_diagnostics.rs:588-589`. The
   report did not verify that citation. **(not determined)**

5. **`THRUST_RESEARCH.md` was not read.** It sits in this same planning
   directory. It may already contain the gantry-push model this survey
   reports as absent from core. The absence claim in section 3.5 is a
   claim about **shipped core code only**.

6. **The `sample_step_mm` at the GUI default is not known.** Core clamps
   it to `request.resolution.max(0.25)` (`compute/simulate.rs:867`). The
   report did not find where `request.resolution` gets its default
   value, because that lives outside core. **(not determined)**

7. **No time-weighted order statistic exists anywhere in the crate.**
   Both sorting sites (`finish/spiral_finish_compact.rs:813`,
   `tool_load/chipload.rs:889`) sort raw values and ignore
   `segment_time_s`. A time-weighted median or percentile would be new
   machinery. This is a statement about what is absent, and the report
   verified it by searching for `percentile`, `quantile` and `median`
   across `crates/rs_cam_core/src`.
