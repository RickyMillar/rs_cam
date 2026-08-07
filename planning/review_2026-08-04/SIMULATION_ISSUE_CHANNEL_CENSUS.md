# SIMULATION_ISSUE_CHANNEL_CENSUS — M1 / R4 (W5)

Date: 2026-08-04. Branch: `experiment/adaptive-spiral`.
Scope: **research only.** No gate retune, no channel redesign, no severity
default moved. Everything below is a census, a measurement, or a proposal
awaiting **Checkpoint D**.

Instrument: `crates/rs_cam_core/tests/simulation_issue_channel_m1.rs`
(test-only, three tests). Fixtures: `test_data/ux_2d_pocket.toml` (synthetic
2D) and `crates/rs_cam_core/tests/fixtures/test_job.toml` (committed
`terrain.stl` + `rivers_aligned.dxf`). **`planning/airrun_2026-06-01/wanaka.toml`
was never opened, edited, staged, or simulated** (programme rule 9).

---

## 0. The headline

1. The `~99,000 issues` figure is **not** one issue per out-of-material
   sample — core coalesced that away in April (`simulation_cut.rs:796-805`).
   It is the count of *contiguous air/low-engagement runs*, which is
   unbounded and grows with how finely a real cut alternates in and out of
   material.
2. **Three different quantities ship under the name "issue count"** —
   coalesced segments, per-sample tallies, and a post-filter MCP count —
   and a single `get_cut_trace` response can display all three at once with
   magnitudes an order apart.
3. There is a **hard, fixed measurability floor at 0.05 mm** in the
   stamping kernel: a pass that removes less than that per stamp reads
   radial engagement of *exactly* zero and is reported as ~96% air cut
   while removing material perfectly well. Measured, below.
4. **The Rivers B4 spike reproduces on a committed fixture and is not an
   engine defect.** A `project_curve` op reads 20.4× its commanded surface
   offset because 5.20 mm of upstream stock is standing above the cutter
   there. The peak sample is `FinishingCut`, `in_transit_span == false`,
   with no `Entry`/`LinkBridge`/`DressupArtifact` ancestry — the
   transit/lift-bridge hypothesis is refuted by four independent tags.
5. Every arc-fitted move is currently **dropped from the chipload,
   deflection and power gate populations** and from `peak_axial_doc_mm` /
   `peak_chipload_mm_per_tooth`, because arc-fit tags each fitted arc with
   a `SpanKind::DressupArtifact` span and that kind is on the transit list.
   The stated rationale for that list never mentions arc-fit.

---

## 1. Typed producer / consumer inventory

### 1.1 Producers

| # | Producer | Site | Emits | Population rule | Bound |
|---|---|---|---|---|---|
| P1 | Air/low-engagement segment builder | `crates/rs_cam_core/src/simulation_cut.rs:807-877` | `SimulationCutIssue` (kinds `AirCut`, `LowEngagement` only — `:138-141`) | per cutting sample: `radial_woc_fraction < 0.02` → AirCut, `< 0.10` → LowEngagement, gated off entirely for `metrics_not_applicable` toolpaths (`:829-831`) | **coalesced by contiguity per toolpath; count unbounded** |
| P2 | Per-sample air/low tally | `simulation_cut.rs:1150-1157` (`SummaryAccumulator::observe`) | `air_cut_issue_count`, `low_engagement_issue_count` on `SimulationToolpathCutSummary` (`:977-978`) and `SimulationSemanticCutSummary` (`:460-461`) | same two thresholds, **one increment per sample** | unbounded |
| P3 | Hotspot accumulator | `simulation_cut.rs:790-816`, `:908-918` | `SimulationCutHotspot` | one per `(toolpath_id, semantic_item_id)`; sorted by `wasted_runtime_s` desc | **unbounded** (one per semantic item) |
| P4 | Rapid-through-stock scan | `crates/rs_cam_core/src/collision.rs:450-544` | `RapidCollision` | tip point sampled every 1 mm along each rapid (`:514`) vs frozen `top_z_at` snapshot | unbounded (one per rapid move) |
| P5 | Holder/shank collision sweep | `collision.rs` (`CollisionKind::Fixture` etc., `:395-411`) | `Collision` | one obstacle hit per segment per sample (`break` at `:408`) | unbounded |
| P6 | Generation debug annotations | `crates/rs_cam_core/src/debug_trace.rs` → `ToolpathDebugTrace::annotations` | generation-time notes | opt-in via `ToolpathDebugOptions` | unbounded |
| P7 | Project verdicts | `crates/rs_cam_core/src/session/compute.rs:3247-3306` | `Verdict { AirCut, RapidCollision, HolderCollision, PlungeStress, GeneratedEmpty }` | air-cut verdict fires per **toolpath** over `OperationType::air_cut_high_threshold_pct` | one verdict per kind; offender list unbounded inside one headline |
| P8 | Drill-native | `crates/rs_cam_core/src/drill_metrics.rs`, `tool_load/drill_gates.rs` | `DrillSample`, `DrillToolpathSummary`, `drill_gates` | per peck / per toolpath | bounded by hole count |

**Not a producer:** `narrate.rs`. It reads no issue, no `issue_count` and
no simulation hotspot. Its "hotspots" line (`narrate.rs:347`) is
`ToolpathDebugSummary::hotspot_count` — a *generation-time* profiling
counter from `debug_trace.rs:108`, unrelated to `SimulationCutTrace::hotspots`.
It rebuilds its own engagement view from `trace.samples` with the same
0.02/0.10 breakpoints (`narrate.rs:437-497`).

### 1.2 Reducers

| Reducer | Site | Rule |
|---|---|---|
| Contiguity coalescing | `simulation_cut.rs:841-877` | same-kind adjacent samples of one toolpath merge into one segment; kind change or good engagement flushes; open segments flushed at stream end |
| Drill suppression (P4 RCA) | `simulation_cut.rs:829-831` | toolpaths in `metrics_not_applicable_toolpath_ids` emit **no** issues at all |
| Supersession | `crates/rs_cam_core/src/diagnostics/supersession.rs` | drops heuristic `Diagnostic`s when Current verified evidence covers the same dimension. **Never applied to issues** — issues are not `Diagnostic`s |
| GUI kind partition | `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:576-602` | "Must address" = `{RapidCollision, HolderCollision, Hotspot}`; "Informational" = `{LowEngagement, AirCut}`; **`Annotation` is in neither and is silently dropped** |

There is **no deduplication anywhere** — not by kind, not by region, not
spatially. Coalescing is a run-length encode, not a dedup.

### 1.3 Consumers and serialization routes

| Route | Site | What it reads | Cap / truncation |
|---|---|---|---|
| GUI issue list | `crates/rs_cam_viz/src/state/simulation.rs:1485-1626` `SimulationState::issues()` | concatenates **five** sources: debug annotations, debug hotspots, `cut_trace.issues`, `checks.rapid_collision_move_indices`, `checks.collision_report.collisions` | **none.** No `.take`, no `.truncate`, no dedup. Sorted by `move_index` primary, `issue_kind_rank` only as a tiebreak (`:1609-1614`) — and that rank puts collisions **last** (`:2184-2193`) |
| GUI issue cache | `simulation.rs:183-196`, `:1624` | caches the full Vec; key = cut-trace Arc ptr + `gui.edit_counter` + debug fingerprint + max feed + collision fingerprint | unbounded; **deep-cloned on every call** (`:1489`), three callers per frame |
| GUI Inspector | `sim_diagnostics.rs:603-714` | must-address counts shown as numbers; informational counts shown only as **time percentages**, raw count on hover with the text "a per-sample emission tally, not a defect count" (`:706-710`) | hotspots `TOP_N = 10` (`:753`); span findings `MAX_ROWS = 8` (`:1499`) |
| GUI banner | `sim_diagnostics.rs:215-257` | `checks.total_collision_count()`, then `cut_trace.summary.air_cut_pct_of_total_runtime() > 20.0` | n/a |
| GUI timeline | `crates/rs_cam_viz/src/ui/sim_timeline.rs:101-110` | builds the whole list, keeps **only** `Hotspot`, discards the rest — with a comment naming "tens of thousands" and a headline pill reading "issues 46751" as the reason | markers: one 2.5 px circle per `cut_trace.issues` entry, **no decimation** (`:2054-2074`) |
| GUI op list | `crates/rs_cam_viz/src/ui/sim_op_list.rs:948-1017` | per-(toolpath, kind) counts for `{RapidCollision, HolderCollision, Hotspot, Annotation}`; AirCut/LowEngagement deliberately excluded | shows `flags.first()` + `+N`; detail `.take(3)` |
| GUI preflight | `crates/rs_cam_viz/src/ui/preflight.rs:86` | **not the issue channel** — the word "issues" there means `checks.holder_collision_count` | n/a |
| MCP `run_simulation` | `crates/rs_cam_viz/src/controller/events/compute.rs:1823-1832` | `issue_count = ct.issues.len()` (unfiltered coalesced segments) + the same `>20%` verdict | n/a |
| MCP `get_cut_trace` | `crates/rs_cam_viz/src/app/mcp.rs:1355-1441` | filters by toolpath + span path; `issue_count` is **post-filter, pre-cap**; array `.take(max_i)` default 50 | array capped, count not; **no `truncated` flag**. The response ALSO embeds unfiltered `summary.issue_count` |
| MCP `inspect_spans` | `mcp.rs:1369, 1437` | same pattern | same |
| MCP `narrate_toolpath` | `narrate.rs:1512-1691` | air-cut line only | anomalies `MAX_ANOMALY_LINES = 8` |
| CLI report | `crates/rs_cam_cli/src/main.rs:328-353` | raw per-toolpath filtered counts, printed as "`N air cuts, M low engagement`" | none; hotspots `.take(10)` with **no "N more" line** |
| CLI verdict | `crates/rs_cam_cli/src/project.rs:448-466` | `air_cut_pct_of_total_runtime > 40.0` | n/a |
| Typed `Diagnostic` | `diagnostics/adapters/from_project_diagnostics.rs:20-80` | **verdicts only**, never issues. `AirCut → "project.air_cut_high"`, `Category::Efficiency`, `Severity::Hint`, evidence = a count of offending *toolpaths* | none |

### 1.4 Count aggregation rules — the three "issue counts"

| Name | Site | Population | Magnitude on the measured fixture |
|---|---|---|---|
| `SimulationCutSummary::issue_count` | `simulation_cut.rs:496`, set from `issues.len()` at `:923` | **coalesced segments** | **821** |
| `air_cut_issue_count` + `low_engagement_issue_count` | `simulation_cut.rs:977-978`, `:460-461`, incremented at `:1153`/`:1156` | **one per sample** | **35,287** |
| MCP `get_cut_trace` top-level `issue_count` | `mcp.rs:1369` | post-filter segments | ≤ 821 |

A 43× gap under three near-identical names, on one trace.

---

## 2. What the ~99,000 actually is — measured decomposition

Instrument: `issue_channel_census_synthetic_2d`, `test_data/ux_2d_pocket.toml`,
cell 0.5 mm, three toolpaths (one real pocket, one identical re-cut over
already cleared ground, one profile). Verbatim output:

```
trace: 51275 samples, 3 toolpaths, summary.issue_count 821 (COALESCED SEGMENTS), hotspot_count 96
project air cut: 69.7% of total runtime / 72.8% of cutting time

toolpath            samples  cutting   airSEG   lowSEG  airSAMPLE  lowSAMPLE   maxSeg
pocket-real           24340    20328      445      314      13823        471      605
pocket-recut-air      24340    20328       42        0      20328          0      819
profile                2595     2379       18        2        663          2      294
```

Reading:

- **Coalescing is working, and it is not enough.** 35,287 flagged samples
  became 821 segments (43:1). But 821 is still an unbounded list, and it is
  the number every MCP/CLI surface publishes.
- **The all-air toolpath is the cheap case, not the expensive one.**
  `pocket-recut-air` is 100% air (20,328 of 20,328 cutting samples) and
  produces only **42** segments, because its air is one long contiguous run
  (max segment 819 samples). An all-air trace does *not* explode the list.
- **The expensive case is a real cut.** `pocket-real` — a perfectly normal
  6 mm pocket — produced **445 air segments + 314 low-engagement segments**
  from 13,823 + 471 flagged samples. Every time the cutter leaves and
  re-enters material (stepover turnarounds, island passes, corner relief)
  the run breaks and a new segment is opened. **Segment count scales with
  path fragmentation, not with how much air there is.**
- Therefore the plan's premise needs one correction, and it matters for the
  fix: *the channel is not dominated by "every out-of-material sample"; it
  is dominated by **transition density in ordinary cutting**.* A cap or a
  spatial dedup will work; more aggressive contiguity coalescing will not.
- 96 hotspots on three toolpaths — one per `(toolpath, semantic_item)`,
  unbounded in the same way.

**Extrapolation to the reported ~99,000, stated as an inference not a
measurement.** Wanaka is out of bounds for this wave, so the figure is not
re-measured here. The structural account that fits the recorded GUI
evidence is: `SimulationState::issues()` concatenates five sources with no
cap, and its own source comments record **24,800** air-cut entries beside
14 hotspots (`sim_diagnostics.rs:576-581`) and a headline pill reading
**46,751** (`sim_timeline.rs:101-107`). Those are *coalesced* segments plus
annotations plus hotspots plus collisions, on a project with roughly 30×
this fixture's move count and far more Z levels and regions. A ~99,000
figure at that scale is consistent with 43:1 coalescing on a fragmented,
many-op project, and is **not** consistent with "one issue per air sample"
(which would be in the millions). Anyone re-measuring should record the
per-source split, which no surface prints today.

---

## 3. Proposed taxonomy — AWAITING CHECKPOINT D

The repository already owns the target contract:
`crates/rs_cam_core/src/diagnostics/mod.rs` — `Diagnostic { id, scope,
category, severity, confidence, state, source, message, evidence, fix,
supersedes }` with four deliberately orthogonal axes (`mod.rs:36-51`). The
proposal is to route the simulation channel into **that** contract, not to
invent a second one.

### 3.1 Five classes

| Class | Definition | Members today | Proposed `Severity` / `Category` | Retention |
|---|---|---|---|---|
| **A. Safety/error** | Physical damage if run | `RapidCollision`, holder/fixture `Collision` | `Critical` / `Safety` | **never dropped, never deduped across distinct events** — one entry per collision move |
| **B. Action-required finding** | Costs a run or a part; the operator must decide | air cut over the op's `air_cut_high_threshold_pct`; `PlungeStress`; `GeneratedEmpty`; drill gate `Exceeds` | `Caution` / `Efficiency`\|`Safety`\|`ToolLoad` | one per (toolpath, rule); evidence carries the worst instance |
| **C. Bounded advisory** | Worth seeing, capped | top-N hotspots by `wasted_runtime_s`; `LowEngagement` rolled to a per-toolpath time share | `Hint` / `Efficiency` | **hard cap** (proposal: 10 per toolpath, 50 per project) + "N more" |
| **D. Diagnostic sample** | Raw evidence, addressable, not a list item | `SimulationCutSample`, `SimulationCutIssue` segments, `DrillSample` | not a `Diagnostic` at all | stays on `SimulationCutTrace`; reachable by query (`get_cut_trace`), never rendered as a user list |
| **E. Noise-by-construction** | Emitted because the model has no other way to say "nothing here" | air-cut samples on `ProjectCurve`/`Drill`; every sample of a pass below the 0.05 mm floor (§5) | **suppressed at source**, replaced by one measurability finding (§5.4) | not retained |

### 3.2 Dedup key

```
(kind, toolpath_id, semantic_source_region, spatial_bucket)
```

- `kind` — the `DiagnosticId` string, not the enum, so new rules do not
  need a wire change.
- `toolpath_id` — already on every issue.
- `semantic_source_region` — **`SpanId` of the nearest `SpanKind::Region`
  ancestor** in `span_path`, plus its `RegionSpanRole`
  (`toolpath_spans.rs:246-259`, `:293-312`). This is a *source* key, which
  satisfies programme rule 5: it survives arcfit and TSP because it is
  structural ancestry, not a post-transform label. Fall back to
  `semantic_item_id`, then to `None`.
- `spatial_bucket` — `(floor(x/B), floor(y/B), floor(z/B))` with
  `B = max(4 × tool_diameter, 10 mm)`. Tool-relative so one bucket is a
  recognisable place on the part at any tool size.

Merge rule: keep **worst severity**, keep the **worst evidence** (max
`axial_engagement_mm` / min `min_radial_engagement` / longest
`duration_s`), sum `sample_count`, keep first and last move index, and
carry an `occurrences` field. Class A is exempt: two collisions in one
bucket stay two entries.

### 3.3 Bounded retention

| Class | User-facing cap | Raw retention |
|---|---|---|
| A | none | full |
| B | none (one per rule × toolpath is already bounded by the rule set) | full |
| C | 10/toolpath, 50/project, with an explicit `truncated: true` + `total_matching` | full on the trace |
| D | not listed | full on the trace, capped only on the wire (`max_issues`, already 50) |
| E | not listed | suppressed at source; the suppression itself becomes one measurability finding (§5.4) |

### 3.4 Compatibility plan for raw `issue_count` / `AirCut` consumers

Named consumers that must keep working (from §1.3):

1. `SimulationCutSummary::issue_count` (wire; pinned by a dozen assertions
   in `simulation_cut.rs`'s own test module — `:1564`, `:1707`, `:1747`,
   `:1794`, `:1870-1871`, `:1904` (JSON round-trip fixture), `:2018-2023`,
   `:2089`, `:2158`, `:2594`, `:2644`, `:2673`) — **keep the field, keep
   the value, document it as
   legacy/diagnostic-sample-count.** Add `finding_count` beside it for the
   new bounded list. Precedent: `air_cut_percentage` was kept as a
   documented alias for `air_cut_pct_of_total_runtime`
   (`session/mod.rs:872`) and pinned by `air_cut_denominators_lh1.rs`.
2. MCP `run_simulation.issue_count` and `get_cut_trace.issue_count` —
   unchanged; add `truncated` to `get_cut_trace` (currently the array is
   capped and nothing says so).
3. CLI `print_diagnostics_report` "`N air cuts`" line — unchanged.
4. `air_cut_pct_of_total_runtime` and every threshold reading it
   (`OperationType::air_cut_high_threshold_pct`, GUI 20%, CLI 40%) —
   **untouched.** A channel redesign must not move a gate number; that is
   the plan's rule and this proposal keeps it.
5. `air_cut_issue_count` / `low_engagement_issue_count` — keep, but rename
   in **documentation** to "flagged sample tally" and state the population
   difference at the field. The GUI already says this on hover
   (`sim_diagnostics.rs:706-710`); no other surface does.

### 3.5 Defects found while censusing (report-only, no fix in this wave)

| # | Defect | Site | Why it matters |
|---|---|---|---|
| D1 | `issue_kind_rank` sorts collisions **last** | `simulation.rs:2184-2193` used at `:1609-1614` | severity is a tiebreak under `move_index`, so an operator stepping the issue list with `focus_issue_delta` reaches collisions at random. A second, contradictory rank exists at `sim_op_list.rs:958-963` (collisions first) |
| D2 | LowEngagement help text states the AirCut threshold | `sim_diagnostics.rs` informational row help | says "< 2% of diameter"; LowEngagement is 0.02–0.10 (`simulation_cut.rs:833-836`) |
| D3 | `Annotation` is dropped by the Inspector partition | `sim_diagnostics.rs:584-592` | in neither bucket, so it never renders there |
| D4 | `get_cut_trace` returns two different `issue_count`s | `mcp.rs:1369` vs the embedded `summary.issue_count` at `:1379` | an agent reading a filtered response can pick either |
| D5 | `air_cut_offenders_for_toolpaths` ignores `tc.enabled` | `session/compute.rs:3630-3660` (sibling `plunge_stress_offenders_for_session` at `:3677` does check) | a disabled toolpath with a stale summary can raise a verdict |
| D6 | Offender→id resolution is **by name** | `session/compute.rs:3255-3262` | duplicate toolpath names collapse to the first match |
| D7 | Narration's air-cut ⚠ marker uses the *cutting-time* denominator at 50% | `narrate.rs:38`, `:1614-1618` | every other threshold in the workspace uses total-runtime; the line prints both numbers but the marker is set by the un-thresholded one |
| D8 | `UnifiedFinish` missing from narration's finish-op hint arm | `narrate.rs:1626-1645` vs `catalog.rs:481-484` | gets the empty hint |
| D9 | CLI hotspot list silently truncates at 10 | `cli/src/main.rs:354-376` | no "N more" line, unlike every GUI list |
| D10 | Two shipped code comments describe the channel as **per-sample** when it has been run-length coalesced since April | `sim_timeline.rs:101-105` ("the raw issues() length is dominated by per-sample air-cut / low-engagement emission noise"); `sim_diagnostics.rs:706-710` hover ("N flagged samples — a per-sample emission tally") | the *hover* text is correct (it does count samples); the *timeline* comment is not (`cut_trace.issues` are segments). §2 shows the two populations differ by 43×, so a reader taking either comment as the model of the other will mis-size any fix |

---

## 4. Page-one triage data contract

The question a page-one answer must serve is **"what should I act on?"** —
identically for an operator reading a panel and for an agent reading JSON.
One typed summary, consumed by GUI, MCP, CLI and narration (the plan's
acceptance gate).

```
SimulationTriage {
  measurability: MeasurabilityReport,      // §5 — read FIRST, it qualifies everything below
  safety:        Vec<Finding>,             // class A, uncapped, severity-sorted
  actions:       Vec<Finding>,             // class B, uncapped, severity-sorted
  advisories:    Bounded<Finding>,         // class C, { items, truncated, total_matching }
  counts:        ChannelCounts,            // the diagnostic-sample tallies, explicitly labelled
}

Finding = Diagnostic                        // the EXISTING core contract, unchanged
        + dedup_key: (id, toolpath_id, region: Option<(SpanId, RegionSpanRole)>, bucket: [i32;3])
        + occurrences: usize
        + worst: Evidence                   // the sample that justified it

ChannelCounts {
  flagged_samples_air: usize,               // was air_cut_issue_count
  flagged_samples_low: usize,
  issue_segments: usize,                    // was summary.issue_count — LEGACY, documented as such
  hotspots_total: usize,
  samples_total: usize,
}
```

Ordering rule (one, shared, replacing the two contradictory ranks):
`severity desc → category (Safety, ToolLoad, Geometry, Quality, Efficiency,
State) → occurrences desc → first move index`. `move_index` becomes the
*last* key, not the first.

Operator rendering, top to bottom:
1. **Measurability strip** — "engagement/air-cut NOT MEASURABLE for
   3 operations at 0.5 mm; collision detection valid." One line, always
   present, never a warning colour when everything is measurable.
2. **Safety** — every collision, with move index and position. Never
   collapsed.
3. **Act on** — one row per rule × toolpath with its evidence value and the
   threshold it crossed, naming the denominator.
4. **Advisories** — capped, with "N more".
5. **Counts** — a collapsed footer. This is where `issue_count` lives.

Agent rendering: the same object as JSON. An agent asking "what should I
act on" reads `safety` then `actions` and never has to know that
`issue_count` is a different population from `air_cut_issue_count`.

Acceptance bars this contract is designed to pass (from the plan):
- all-air synthetic trace → `advisories.truncated == true`, bounded list —
  and note §2's finding that the all-air case was never the explosive one;
- one rapid collision + one holder collision + one removal warning stay
  visible beside thousands of air samples — they are in `safety`/`actions`,
  which are never capped and never share a list with class D;
- dedup preserves worst evidence and never merges two distinct class-A
  events (class A is exempt by rule).

---

## 5. Resolution-measurability matrix

### 5.1 The two floors, both measured

**Floor 1 — fresh-material floor (fixed, 0.05 mm, NOT a function of cell size).**
`crates/rs_cam_core/src/dexel_stock/stamping.rs:557`:

```rust
const FRESH_MATERIAL_THRESHOLD_MM: f64 = 0.05;
```

A cell contributes to the radial width-of-cut measurement only if it held
more than 0.05 mm of material above the cutter before the stamp
(`stamping.rs:657`). Below that, `perp_min`/`perp_max` are never updated,
`perp_max > perp_min` is false, and `radial_engagement` is set to **exactly
0.0** (`stamping.rs:675-679`). Since `AirCut` is defined as
`radial_woc_fraction < 0.02` (`simulation_cut.rs:833`), **every cutting
sample of such a pass is classified air cut.**

Measured — `engagement_is_unmeasurable_below_the_fresh_material_floor`,
same geometry, two depths:

```
shallow 0.02 mm: air 95.9% of total runtime, avg engagement 0.0000, peak radial 0.0000,
                 peak removed height 0.0200 mm, removed volume 63.7 mm3
deep    2.00 mm: air 49.6% of total runtime, avg engagement 0.2293, peak radial 0.9173,
                 peak removed height 1.9800 mm, removed volume 6515.9 mm3
```

The shallow arm removes 63.7 mm³ of real material, reports the removed
height correctly (0.0200 mm — the axial channel is fine), and reports
**zero** engagement and **95.9% air cut**. Nothing in any surface says the
number is unmeasurable; it prints as a precise-looking percent and clears
every shipped bar by a wide margin — the GUI's 20% banner, the CLI's 40%
verdict, and (on a real finishing operation, which is where sub-0.05 mm
passes actually live) `OperationType::air_cut_high_threshold_pct`'s 30%
finish band. The measured arm here is a `Pocket` (40% band) because the
control arm had to be the same operation type over the same geometry; the
floor itself is operation-independent.

**Floor 2 — lateral resolvability (a function of cell size).**
The perp-extent measurement also requires `coverage >= PERP_COVERAGE_GATE`
(`0.95`, `stamping.rs:572`), i.e. cells essentially fully inside the swept
disc, and needs **two distinct qualifying cell centres at different
perpendicular offsets** for `perp_max > perp_min` to hold. For a round-tip
tool at cut depth `d` the contact radius is `a = sqrt(2·R·d − d²)`, so the
criterion is approximately

> `cell_size ≲ sqrt(2·R_tip·d − d²)`

A Ø1 mm ball tip (`R = 0.5`) at `d = 0.05 mm` gives `a ≈ 0.218 mm` — a
0.25 mm grid yields at most one qualifying cell and reads zero. This is the
quantitative form of the standing memory rule "sim cell must be well below
the tool **tip** radius".

**Note the plan's stated instance needs refining.** The brief describes
"cell-below-cut-depth". The measured mechanism is two independent
conditions — a fixed 0.05 mm *material* floor (independent of cell) and a
lateral-resolution condition (dependent on cell **and** on tip radius and
cut depth together). Neither is "cell < cut depth".

### 5.2 What stays valid when engagement is unmeasurable

| Metric | Mechanism | Fails when | Still valid when engagement fails? |
|---|---|---|---|
| Rapid collision | tip point every 1 mm vs frozen `top_z_at` (`collision.rs:514-531`) | cell too coarse to hold a thin standing ridge; **also tip-point-only — the shank/flute is never tested** | **YES** — independent of the engagement path |
| Holder/fixture collision | analytic segment vs obstacle | — | **YES** |
| Gross material removal | `pre_len − post_len` per cell, f32 rays (`stamping.rs:607-626`) | never below the floor; Z is continuous | **YES** — the shallow arm reported 63.7 mm³ and 0.0200 mm correctly |
| `peak_axial_doc_mm` / `axial_engagement_mm` | `max(pre_len − post_len)` over the midpoint disc (`stamping.rs:666-669`) | no coverage gate at all | **YES** (but see §6 — it measures standing stock, not commanded depth) |
| `radial_woc_fraction`, `average_engagement` | perp extent of fresh cells | **both floors** | **NO** |
| `air_cut_time_s` and both percentages | derived from `radial_woc_fraction < 0.02` | **both floors** | **NO** |
| `arc_engagement_radians` → chipload | derived from radial engagement (`stamping.rs:680-682`) | **both floors** | **NO** |
| Drill-native (`DrillToolpathSummary`, `drill_gates`) | analytic peck geometry, no dexel stamping | — | **YES** — and already separated by `metrics_not_applicable` |
| Fine cusp / surface quality | `SimulationResult::column_deviations` | its own resolution rules (X-9) | separate instrument; not part of this channel |

### 5.3 Matrix by operation family

Cut-depth column = the per-stamp axial engagement the op typically
produces, which is what Floor 1 tests.

| Operation family | Typical per-stamp axial | Engagement / air-cut | Rapid collision | Removal | Drill-native |
|---|---|---|---|---|---|
| Pocket, Face, Adaptive, Zigzag, Profile, Trace, Inlay, Chamfer (2.5D, dpp ≥ 0.5 mm) | ≫ 0.05 | **Measurable** | Measurable | Measurable | n/a |
| Adaptive3d rough | ≫ 0.05 | **Measurable** (scalar is comparative, per CLAUDE.md) | Measurable | Measurable | n/a |
| VCarve | varies with the V profile; tip passes are ≪ 0.05 | **Degraded** — the tip region is unmeasurable, the flank is not | Measurable | Measurable | n/a |
| DropCutter, Waterline, Scallop, HorizontalFinish, SteepShallow, RampFinish, SpiralFinish, RadialFinish, UnifiedFinish (finish, stepover-limited) | scallop height, routinely 0.005–0.05 mm | **NotMeasurable** below the floor; **Degraded** near it; also fails Floor 2 on small tips at ≥ 0.25 mm cells | Measurable, but tip-point-only and cell-limited | Measurable | n/a |
| Pencil | valley contact, very small | **NotMeasurable** | Measurable | Measurable | n/a |
| ProjectCurve | fixed surface offset; commanded 0.2–0.4 mm but actual = standing stock (§6) | **Degraded** — the reading is real but is about the stock, not the plan | Measurable | Measurable | n/a |
| Rest | depends on the upstream leave | **Degraded** | Measurable | Measurable | n/a |
| Drill, AlignmentPinDrill | Z-only | already `metrics_not_applicable` | Measurable | analytic | **Measurable** |

### 5.4 Proposed reporting (Checkpoint D)

Add a per-(toolpath, metric) measurability verdict, carried as a
`Diagnostic` with `state: DiagnosticState::NotApplicable` (that state
already exists and already means "not a warning" — `mod.rs:226-229`) plus a
new `reason` on the evidence:

```
Measurable
Degraded  { reason, measured_floor_mm, effective_cell_mm }
NotMeasurable { reason, measured_floor_mm, effective_cell_mm }
```

Reasons observed today: `BelowFreshMaterialFloor { removed_mm, floor_mm }`,
`CellTooCoarseForTipContact { cell_mm, contact_radius_mm }`,
`KinematicsNotModelled` (the existing drill case).

Rule: **when a metric is `NotMeasurable`, its percentage must not be
published as a number** — neither `air_cut_pct_of_*` nor
`average_engagement` — and the finding must name which measures remain
valid (collision, removal, drill-native). No threshold changes; a
`NotMeasurable` metric simply stops feeding its gate, which is a policy
change and therefore **Checkpoint D**.

`SimulationResult` already carries the provenance this needs:
`resolution_clamped` and `column_grid_cell_mm` (`compute/simulate.rs:301-317`).

---

## 6. The Rivers B4 probe

### 6.1 What was claimed, and what is already settled

Original observation (`planning/unified_v3_design.md:1919-1943`, 2026-07-28):
`Rivers (back)`, a `project_curve` op on a 20° V-bit with commanded
`depth = 0.4 mm` and `stock_source = from_remaining_stock`, reported
**peak axial DOC 6.07 mm** at (111.7, 39.6), z ≈ 3.884.

**Arc-fit is exonerated and is not re-litigated here.** With
`arc_fitting = false` the value, position and Z were identical (6.07 mm,
(111.7, 39.6), 3.885); only the move representation changed (move 174
`ArcCCW` → move 325 `Linear`).

**`Rivers (back)` exists only in play files.** `wanaka.toml`,
`wanaka_suggested.toml` and three `feed_modulation_calibration/bench_batch_2026-05-27/`
copies — all referencing external absolute paths under
`/home/ricky/Downloads/wanaka100/` that are not in the repo. The exact
6.07 mm value is therefore **not reproducible from committed material**, by
construction, and this wave does not open the play file.

### 6.2 The mechanism is already proven in committed code

`crates/rs_cam_core/tests/axial_doc_step_multiple_h4.rs` establishes, with
a hand-built toolpath and no generator:

- `peak_axial_doc_mm` is `max(pre_ray_len − post_ray_len)` over the
  midpoint-disc cells (`stamping.rs:666-669`) — **the height of material
  the stamp removed.** Nothing in the kernel knows what was commanded.
- `an_operation_with_no_commanded_step_still_reports_a_peak_axial_doc`
  (`:248`) — a single lateral pass with no depth stepping reads the **full
  standing height**.
- `peak_axial_doc_counts_standing_stock_not_the_commanded_step` (`:157`) —
  virgin ground reads exactly 2× where cleared ground reads 1×.

`project_curve` has no `depth_per_pass` at all; its `depth` is a *surface
offset*. So the ratio "6.07 mm against a commanded 0.4 mm" divides a
removed height by a surface offset — two different quantities. That is the
same category error `axial_doc_step_multiple_h4.rs` documents for op 8.

### 6.3 Probe on a committed structural analogue

`rivers_b4_probe_project_curve_on_remaining_stock` runs
`crates/rs_cam_core/tests/fixtures/test_job.toml` — `terrain.stl` +
`rivers_aligned.dxf`, `Project Curve 6` (tool 2 tapered ball, commanded
`depth = 0.2`, `surface_model_id = terrain.stl`,
`stock_source = from_remaining_stock`) downstream of an `adaptive3d` rough
that leaves `stock_to_leave_axial = 5.0`. Everything else disabled. It
groups every cutting sample by removed height / commanded depth (upstream
stock coverage) **and** reports the peak sample's `in_transit_span`, its
`MoveIntent` re-joined through `move_index`, and its span-kind ancestry
(transit/source semantic role), plus the upstream `prior_stocks` height at
the peak XY.

### 6.4 Probe result — **REPRODUCED, and localised to upstream stock coverage**

`cargo test -p rs_cam_core --test simulation_issue_channel_m1 -- --ignored
--nocapture --test-threads=1`, 907.66 s, cell 0.5 mm, verbatim:

```
Project Curve 6: commanded depth 0.200 mm, summary.peak_axial_doc_mm 4.0722 mm (20.4x commanded),
                 peak_plunge_descent_mm 0.0200 mm
air cut 19.9% of total runtime / 28.2% of cutting time; 1054 spans,
kinds: {"DressupArtifact": 630, "Entry": 141, "LinkBridge": 141, "Operation": 1, "Region": 141}

removed-height buckets (cutting samples):
  {"<=1.5x commanded (surface-following)": 50811, "1.5-3x": 38, "3-10x": 574, ">10x (standing material)": 2553}

peak cutting sample: axial_engagement 4.0722 mm (20.4x commanded) at (x=15.97, y=42.25, z=9.800);
  move 492 intent FinishingCut; in_transit_span false; kinematics Linear;
  span kinds ["Operation", "Region"]; radial_woc 0.3147
prior-stock max top within 1 mm of the peak XY: Some(15.0); cutter z 9.800;
  standing above cutter Some(5.199999999999999)
```

**The spike class reproduces on a committed fixture.** 4.0722 mm against a
commanded 0.200 mm surface offset is **20.4×** — the same class as Wanaka's
6.07 mm against 0.4 mm (15×), on the same operation type, the same
`stock_source`, and the same upstream-rough arrangement.

**Grouping by transit / source semantic role: hypothesis REFUTED.** The
peak sample is:

- `in_transit_span == false` — not masked *and* not maskable by the transit
  filter;
- `MoveIntent::FinishingCut` — a source-tagged cutting move, not `Linking`,
  not any `Entry*`, not `LeadOut`;
- span ancestry `["Operation", "Region"]` — no `Entry`, no `LinkBridge`, no
  `DressupArtifact`, despite this op carrying 141 `Entry`, 141 `LinkBridge`
  and 630 arc-fit `DressupArtifact` spans;
- `CutKinematics::Linear` with `radial_woc 0.3147` — a normal, well-engaged
  lateral cut.

There is no lift bridge, no entry transient, and no dressup artifact at the
peak. Lift-function bridging is ruled out for this sample by four
independent structural tags.

**Grouping by upstream stock coverage: hypothesis CONFIRMED.** The
upstream `prior_stocks` snapshot the op consumed reports a stock top of
**15.0 mm** within 1 mm of the peak XY, against a cutter Z of **9.800 mm** —
**5.20 mm of material standing above the cutter** at that point. The
upstream `adaptive3d` rough is configured `stock_to_leave_axial = 5.0`
(`tests/fixtures/test_job.toml`). The measured 4.07 mm removed height sits
just under that standing column, as it must: the tapered ball removes the
profile-limited portion of the column inside the midpoint disc, not the
whole 5.2 mm. **The reading is correct. The curve is cutting through
material the upstream pass deliberately left.**

The bucket distribution shows this is not a single freak sample:
**2,553 of 53,976 cutting samples (4.7%)** removed more than 10× the
commanded offset, and 50,811 (94.1%) sat at or below 1.5× — the
surface-following majority the op is supposed to produce. That is the
signature of a curve repeatedly crossing an un-roughed step, exactly the
condition `unified_v3_design.md:1945-1952` nominated and could not test.

**Verdict: B4 is a planning/reporting condition, not an engine defect.**
`peak_axial_doc_mm` faithfully measures removed height. Two things are
wrong, and neither is the simulator:

1. **`project_curve` has no commanded axial depth to divide by.** Its
   `depth` is a surface offset. Every surface that renders "peak axial DOC
   N mm" beside a commanded number invites a ratio with no denominator —
   the same category error `axial_doc_step_multiple_h4.rs` documents for
   op 8. Narration says so and then invites the comparison anyway
   (`narrate.rs:1470`).
2. **Nothing reports the upstream-coverage fact**, which is the actionable
   one: *this finishing pass crosses material the rough left standing.*
   That belongs in the taxonomy as a class-B action-required finding
   (Category `Geometry` or `ToolLoad`), with the standing height and the
   fraction of samples over the bar as evidence. It is currently invisible.

**Sub-finding, and a caution for §7.** This op carries **630 arc-fit
`DressupArtifact` spans**. The peak happened to land outside them, so it
survived the transit filter. Had it landed inside one — a coin toss on a
curve-following op where most moves are arc-fit candidates —
`summary.peak_axial_doc_mm` would have silently skipped it
(`simulation_cut.rs:1134-1141`) while narration's unfiltered peak
(`narrate.rs:1417-1430`) still reported it. **The arc-fit exclusion of §7
can therefore make the two published peaks disagree on exactly this class
of event.** That is a second, independent reason to fix §7 and a concrete
red-first case for it.

**What is NOT claimed.** The specific 6.07 mm value at (111.7, 39.6) is not
reproduced — it belongs to a play file this wave does not open (§6.1). What
is established is the mechanism, on committed material, with the competing
hypothesis refuted by four structural tags and the surviving one confirmed
by a direct upstream-stock reading.

### 6.5 Structural findings from the probe design (independent of its result)

1. **`peak_axial_doc_mm` has no coverage filter and no `is_cutting` filter.**
   `SummaryAccumulator::observe` (`simulation_cut.rs:1134-1141`) applies
   only `!sample.in_transit_span`, and it sits **outside** the
   `if sample.is_cutting` block at `:1147`.
2. **Narration's peak is a different peak.** `append_peak_doc_anomaly`
   (`narrate.rs:1405-1510`) takes `max_by(axial_doc_mm)` filtered only by
   `is_cutting` and `toolpath_id` — **it does not apply the transit
   filter**. So the narrated sample can differ from
   `summary.peak_axial_doc_mm`, and the 6.07 mm figure came from
   narration.
3. **No sample carries `MoveIntent`.** It survives only as the collapsed
   boolean `in_transit_span` (`simulation_cut.rs:198`). Any "source
   semantic role" grouping must re-join through `move_index` against the
   annotated toolpath, as this probe does. There is no MCP route to it.
4. **Nothing groups by upstream stock coverage anywhere.** The dexel does
   carry `grid.coverage_max[idx]` but it is never propagated to a sample.
   `SimulationResult::prior_stocks` is the only upstream-coverage handle
   and no diagnostic reads it.
5. **`axial_engagement_vs_dpp_detector_f2.rs` cannot cover this class** —
   its matrix is Pocket/Profile/Adaptive/Zigzag/Trace and its bar is
   `1.5 × depth_per_pass`, which `ProjectCurve` does not have.

---

## 7. The `DressupArtifact` gate-population verdict (W2 handoff)

### 7.1 Correction to the handoff premise

The channel is **`SpanKind::DressupArtifact`**, not `MoveIntent`.
`MoveIntent` has no such variant (`crates/rs_cam_core/src/toolpath.rs:53-80`:
`Drilling, EntryPlunge, ClearingCut, FinishingCut, EntryHelix, EntryRamp,
Linking, Retract, LeadIn, LeadOut, Unknown`). W2's intent-key fixes
(`3dbec75`, `5fad7e2`) did not touch the span channel, and the exclusion
described here predates and survives them.

### 7.2 The chain, confirmed

1. `crates/rs_cam_core/src/arcfit.rs:280-284` pushes one
   `Span::new(pos, pos+1, SpanKind::DressupArtifact).with_label("arc-fit")`
   **per emitted arc** (`arc_positions.push(arc_idx)` at `:261`). Module
   doc `:44` states it outright.
2. `toolpath_spans.rs:557-580` `transit_moves_bitmap()` lists
   `DressupArtifact` among the transit kinds.
3. `dexel_stock/simulation.rs:178-179` sets
   `in_transit_span = transit_moves[move] || move_type == Rapid`.
4. `tool_load/locality.rs:180-207` `is_phantom_transit` returns true on
   `DressupArtifact` ancestry **or** on the flag.
5. `is_steady_state_for_gate` (`locality.rs:149-154`) therefore excludes
   the sample.

Arc-fitting is **default-on for all three process roles**
(`compute/config.rs:1489`, `:1501`, `:1508`).

### 7.3 Which gates and metrics lose those samples

| Consumer | Site | Predicate | Effect on arc-fitted samples |
|---|---|---|---|
| Chipload trip loop | `tool_load/chipload.rs:582` | `is_phantom_transit` → `continue` | **dropped** (after `valid_count += 1` at `:574` — so `valid_count` counts samples the loop then discards) |
| Chipload LUT `lookup_axial_doc_mm` fold | `chipload.rs:386-390` | `is_steady_state_for_gate(s, span_lookup)` | **dropped** |
| Deflection gate | `tool_load/deflection.rs:240` | `is_phantom_transit` → `continue` | **dropped** |
| Power gate | `tool_load/power.rs:222` | `is_phantom_transit` → `continue` | **dropped** |
| Viewport chipload band precompute | `tool_load/mod.rs:236-244` | `is_steady_state_for_gate(s, None)` | **dropped** (via the flag) |
| `SummaryAccumulator` peak DOC + peak chipload | `simulation_cut.rs:1134-1141` | `!in_transit_span` | **dropped** |
| `KinematicsAccumulator` peak DOC | `simulation_cut.rs:1051-1055` | `!in_transit_span` | **dropped** |
| `is_bipolar_engagement` | `chipload.rs:201-224` | **none** | kept — sees a different population from the trip loop beside it |
| `optimize/context.rs:77-84` median RPM | `.filter(is_cutting)` only | kept | same inconsistency |
| Time-weighted means (`average_engagement`, arc, chip thickness), `peak_plunge_descent_mm`, `peak_mrr_mm3_s`, `air_cut_time_s` | `simulation_cut.rs:1120-1160` | **none** | kept |
| Drill metrics | `drill_metrics.rs` | none; own sample type | unaffected |

### 7.4 Is the exclusion intended for any of them?

**For the dogbone dressup (`dressup.rs:1156`), yes.** The stated rationale
— "the dexel reads `stock_top − cutter_z` over *neighbouring* uncleared
stock the bridge crosses" (`locality.rs:159-166`, `:130-137`,
`toolpath_spans.rs:551-556`, `planning/archive/P3_TRANSIT_PEAK_DOC_RCA.md:32,51`)
— describes exactly a corner-relief motion that flies over adjacent stock.

**For arc-fit, no rationale exists.** The RCA's only phrase covering it is
"dressup-introduced replacement segments" (`P3_TRANSIT_PEAK_DOC_RCA.md:51`).
A fitted arc is not a bridge: it is the *same cut*, re-represented within
`arc_tolerance` (default 0.05 mm), engaging the same material. `git blame`
puts the `DressupArtifact` term at `7d01311` (2026-06-07), whose message
names the four kinds as one list without distinguishing them. The
classification is over-broad **by construction, not by decision**.

Two further consequences, both stated rather than fixed:

- The classification **flips on `spans_valid`.** When spans are invalid,
  arcfit pushes no spans at all (`arcfit.rs:285-287`), and the intent
  fallback bitmap does not mark arcs transit
  (`toolpath_spans.rs:590-592` documents this). So the same toolpath's
  arcs are in the gate population or out of it depending on whether an
  earlier transform invalidated spans.
- **Scope is real.** Only moves that actually collapse into arcs are
  tagged, but arc-fit is default-on and on curve-heavy finishing paths the
  fitted fraction approaches the whole cut. Measured here: the committed
  `Project Curve 6` fixture carries **630 arc-fit `DressupArtifact` spans**
  on one op (§6.4). The handoff's wanaka-scale figure (~11,346 arcs on
  Op B) is the same phenomenon an order of magnitude up; it is quoted from
  the W5 brief and is not re-measured in this wave.
- **The direction of the effect is counter-intuitive and is already
  predicted.** `ARCFIT_INTENT_EVIDENCE.md` §5 item 5: *more* arcs ⇒ more
  `DressupArtifact` spans ⇒ **more** samples dropped by
  `is_phantom_transit`. W2 flagged this as "the consequence most likely to
  be misread as a regression." It is the same mechanism, seen from the
  other side.

### 7.5 Proposed correct predicate — AWAITING CHECKPOINT D

The canonical predicate stays `locality::is_steady_state_for_gate`; what
changes is what feeds it.

**Recommended (P1): split the span kind.** `SpanKind::DressupArtifact` is
carrying two incompatible meanings. Give arc-fit its own kind — e.g.
`SpanKind::GeometryRefit` — or, cheaper and wire-safe, key on the existing
`label` (`"arc-fit"` vs `"dogbone"`) inside `transit_moves_bitmap()` and
`is_phantom_transit`. A `SpanKind` addition touches `SpanKind::ALL`
(`toolpath_spans.rs:196-206`), MCP span serialization
(`viz/src/app/mcp.rs:4824`, `:4890`), the overlay colour tables
(`stock_mesh.rs:194`, `sim_op_list.rs:731-759`, `viewport_overlay.rs:143`)
and `fingerprint.rs:463`. Label-keying touches two functions.

*Do not* simply delete `DressupArtifact` from the transit list: that would
re-admit dogbone bridge samples, which the P3 RCA removed for cause.

**Red-first evidence the fix must carry** (plan rule 13): a fixture with a
curve-heavy cut, generated twice — `arc_fitting` on and off — asserting
that the chipload/deflection/power gate **populations** (sample counts, not
verdicts) match within a stated tolerance. On the parent revision the
arc-fitted arm must show a materially smaller population. Note this is a
*population* bar, not a verdict bar: a verdict bar can pass vacuously
because dropping samples usually lowers a peak.

**Second item, independent of the fix:** `chipload.rs:574` increments
`valid_count` before the `is_phantom_transit` `continue` at `:582`, so
`valid_count` overstates the population that actually drove the gate.
Report-only.

---

## 8. Checkpoint D decision list vs what is report-only implementable

### 8.1 Needs a Checkpoint D ruling (policy / gate / severity / threshold)

| # | Decision |
|---|---|
| D-1 | Does a `NotMeasurable` metric **stop feeding its gate**? Today a 0.02 mm finishing pass reports 95.9% air cut and trips the 30% finish threshold. Suppressing that changes which verdicts fire |
| D-2 | Severity/category assignment for the migrated issue classes (§3.1) — in particular whether `AirCut` over an op's threshold stays `Severity::Hint` (`from_project_diagnostics.rs`) or becomes `Caution` |
| D-3 | Advisory caps: 10/toolpath, 50/project, or other. Any cap changes what an operator sees |
| D-4 | Whether the arc-fit `DressupArtifact` exclusion is corrected (§7.5), and by which mechanism (new `SpanKind` vs label-keying). This **changes gate populations**, therefore verdicts |
| D-5 | Whether narration's air-cut ⚠ marker moves to the total-runtime denominator (D7) — it changes when a ⚠ appears |
| D-6 | Whether the `issue_kind_rank` ordering inverts (D1) — it changes what an operator reaches first |
| D-7 | Whether `FRESH_MATERIAL_THRESHOLD_MM` is a *tunable* or a documented limit. Lowering it re-admits float-noise cells; the census recommends **documenting, not tuning** |
| D-8 | Whether the air-cut verdict starts respecting `tc.enabled` (D5) — it changes which verdicts fire |

### 8.2 Report-only, implementable without a ruling

| # | Item | Why it is safe |
|---|---|---|
| R-1 | Add `truncated` + `total_matching` to `get_cut_trace` / `inspect_spans` | additive wire field; today the array is capped silently |
| R-2 | Resolve the two `issue_count`s in one `get_cut_trace` response (D4) by naming the embedded one | rename of a nested key's documentation, or add `summary_issue_count_project_wide` |
| R-3 | Fix the LowEngagement help string (D2) | pure text, currently wrong |
| R-4 | Add `Annotation` to the Inspector partition (D3) | currently dropped entirely; adding it to "Informational" surfaces nothing new numerically |
| R-5 | Add `UnifiedFinish` to narration's finish-op hint arm (D8) | text only |
| R-6 | Add "N more" to the CLI hotspot truncation (D9) | text only |
| R-7 | Resolve air-cut offenders by `ToolpathId` not name (D6) | fixes a collapse; cannot add a verdict that was not already firing |
| R-8 | Emit the measurability **finding** (report-only, `DiagnosticState::NotApplicable`) without yet suppressing any metric | adds information, changes no gate — this is the plan's own "add a measurability finding before considering a warning/refusal" |
| R-9 | Publish `ChannelCounts` with its three populations explicitly labelled | additive; the existing fields keep their values |
| R-10 | Document `valid_count`'s overstatement at `chipload.rs:574` | comment only |
| R-11 | Carry `MoveIntent` (or a source-role enum) on `SimulationCutSample` | additive `#[serde(default)]` field; unblocks source-role grouping for every future probe. **Population-at-source (rule 5) currently has no per-sample handle at all** |
| R-12 | Report the §6.4 upstream-coverage fact as a class-B finding ("this pass crosses N mm of material the upstream op left standing; X% of samples over the bar") | additive finding; the data (`prior_stocks`, removed height) already exists and nothing reads it. This is the actionable half of B4 |
| R-13 | Stop rendering "peak axial DOC vs commanded" for ops with no commanded axial step (`ProjectCurve`, surface finishes) | narration already prints "commanded depth_per_pass is unknown" and then invites the ratio anyway (`narrate.rs:1470`); removing the invitation changes no number |

### 8.3 Sequencing note

R-8 and R-11 should land first: the measurability finding is what makes
D-1 decidable with evidence rather than argument, and the per-sample source
role is what makes D-4's red-first population bar expressible.

---

## 9. NOT RUN / NOT MEASURED, stated

- **Wanaka `~99,000` not re-measured.** Play-file rule. §2's decomposition
  is an inference from the synthetic ratio plus the two recorded GUI
  figures (24,800 and 46,751) and says so.
- **The viz-side `SimulationState::issues()` count was not measured**, only
  read. It is a five-source concatenation with no cap; measuring it needs a
  `rs_cam_viz` harness, which this wave did not build.
- **The exact 6.07 mm value is not reproduced** and cannot be from
  committed material (§6.1). §6.3's probe establishes the mechanism class
  on a committed analogue.
- **No gate, threshold, severity default, or cap was changed.**
- **Floor 2's closed form (`cell ≲ sqrt(2·R·d − d²)`) is derived, not
  swept.** A resolution sweep confirming it is a natural follow-up and was
  not run.
