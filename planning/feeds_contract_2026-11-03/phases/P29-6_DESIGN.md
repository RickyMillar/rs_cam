# P29-6 design: the gates judge every sample that removes material, by its cut kind

Written 2026-11-12 on HEAD `d84ca2e76`. No product code changed. The lead reviews this note,
commits it with the two prediction files, and then starts an implementer. The result of each
commit is judged against section 10. A mismatch stops the work.

Files of this step:

- this note;
- `P29-6_predictions_fm1.csv`: every FM1 cell value that moves, per commit (columns `commit`,
  then the P29-4 columns). 22 rows, all in the simulation file. A cell value that is not in this
  file must not move.
- `P29-6_predictions.csv`: the four simulated fixtures of the sentry
  `a_sample_states_its_cut_kind_p29_3`. Table `K`: per kind and per group, per gate, the samples
  that each gate judges today and after, with the values. Table `V`: the headline verdicts that
  move, per commit.

Rulings in force: R4 (operator, 2026-11-11: "Entries and cleanup are judged like any cut, as
their own group"; "They warn like any cut, shown as their own group. The relative 'entry load'
rule goes when the absolute one exists"). R1 (the force budget, 35.3224 N on the default
machine). K6 (no finding refuses an export). No legacy (2026-09-16): a rule, a field or an enum
member that no reader reads after the step is deleted, and the commit names the break.

## 1. Scope

### 1.1 The sources, and where they disagree

| Source | What it puts in P29-6 |
|---|---|
| Change list (`P29_FINDINGS.md` section 6) | "The sample holds its cut kind. The gates judge entries, cleanup and capped steps, grouped by kind. The 95 % feed filter goes. The depth gate judges every sample." Acceptance: no sample that removes material is outside the gates. |
| Ruling R4 | entries and cleanup, each as their own group |
| Rows whose `closes_in` names P29-6 (16) | `P28-21` (entries), `P28-22` (waterline cleanup), `P28-23` (lead-out, link, dogbone), `P28-29` (the modulator skips leads), `M1` (95 % filter), `M2` (the modulator caps entries that no gate judges), `M3` (entry samples go to `entry_spike`), `M4` (two sample depths, "a probe"), `M5` (the depth gate reads one sample), `M8` (readers with no span lookup), `U-ResidueCleanup` ("P29-6 / P29-7"), `Q10-LinkingInEntrySpan`, `Q10-EntryPlungeInNoSpan`, `Q4-SlotRingInferred` ("a planner marker"), `Q4-CappedStepNoTrace`, `C-LeadInApproach` |
| Hand-over, "P29-3 status" | the readers that WAIT for body facts in `ToolpathLoadContext` and `FeedsInput` ("do it at P29-6") |

The disagreements:

1. **Capped steps are judged today.** The span filter calls a `CappedStep` and a `SlotRing`
   sample steady (`CutKind::is_judged_today`, `limits/sample.rs:259-266`; the sentry holds it
   equal to the filter). Only the 95 % feed filter of the chip gate drops them
   (`tool_load/chipload.rs:230-243`): on the shelf fixture 339 capped samples, on the Pocket
   fixture 390 slot-ring samples. "Judge capped steps" is therefore the M1 change, not a
   separate one.
2. **R4 does not name lead-outs, links and dogbone reliefs.** Row `P28-23` says "the operator
   rules at P29-6 how such a sample is judged". The code calls their readings phantom (the
   dexel reads neighbour stock). On the four fixtures the probe of section 10 does not support
   that claim for links, lead-outs and cleanup, at either sample depth (`axial_doc_mm` and
   `axial_engagement_mm` give the same ratios there). This is question Q1.
3. **`M4` and `U-ResidueCleanup` are not gate scope.** M4: on every fixture the depth predicate
   of 6c and the force reading give the same peak force (FM1 Pocket softwood 25.2 N both, shelf
   19.25 / 19.236 N, FM1 Adaptive EndMill hardwood 13.67 / 13.670 N), so P29-6 needs no decision on the
   two depths; the difference shows on ball and V tools, which have no budget in these
   fixtures. Residue cleanup: its moves carry `ClearingCut` and lie in the depth pass, so they
   are `Body` samples and the gates judge them today; the row is about the declaration before
   the simulation. Both rows get `closes_in` "P29-7" (text only, no pin moves).
4. **The two Q4 rows are labels, not exemptions after P29-6.** A slot ring and a capped step
   with no trace both fall in the body group, so every gate judges them. Only the label in the
   per-kind breakdown is inferred. They close; the label gap goes to the open items of
   `limits/CLAUDE.md`.
5. **Three of the seven WAIT readers do not wait any more.** P29-5 moved `tool_load/depth.rs`,
   `tool_load/cut_force.rs` and `force_budget::rigidity_depth_cap` onto
   `OperationType::body_depth_limit()`, which the gate context reaches through
   `operation_kind`. Section 1.3 decides the other four.

**Proposed scope:** the change list row as written, with ruling R4, plus the modulator half of
the rows that list modulator files (`P28-22`, `P28-23` and `M2` hold markers in
`session/compute.rs`, `P28-29` in `dressup/feed_modulation.rs`): the modulator reads the same
samples that the gates judge. The envelope finding (`compute::simulate::envelope_placement`)
keeps the body group only; row `P28-21` narrows to it and closes in P29-8 (Q4).

### 1.2 The definition of "removes material"

Two definitions exist today:

- `SimulationCutSample::removes_material()` (`stock/simulation_cut.rs:291-302`, "The one
  definition of 'removing'"): `is_cutting && removed_volume_est_mm3 > 0`.
- `Cut::of_sample` returns `None` on `is_air()` (`limits/sample.rs:300-303`): not cutting, or
  radial width under 0.02 D.

They differ on the air-class samples that remove material (the flank cuts a skin above the tip,
G-WLAIRLABEL). The probe counts them: Pocket 94 of 1204 body removing samples, shelf 2665 of
7089, FM1 Adaptive EndMill 5160 of 6115, FM1 DropCutter EndMill 45551 of 45551. No sample is
engaged and removes nothing (the count is 0 on every fixture), so `removes_material()` holds
every sample that `!is_air()` holds.

Proposal: the acceptance and the gates read `removes_material()`. `Cut::of_sample` returns
`None` exactly when the sample removes nothing. An air-class removing sample becomes a cut
with `CutEngagement::NoModel(NoEngagementModel::NoMeasuredArc)` when the gate door gives no
engagement, and the gates keep their air test as a reason for no number. Such a sample is
judged and counted as `NoNumber`, never skipped. The predictions assume this (Q2).

### 1.3 The readers that WAIT: which ones belong to P29-6

| Reader | Reads today | P29-6? | Why |
|---|---|---|---|
| `tool_load/depth.rs`, `tool_load/cut_force.rs`, `force_budget::rigidity_depth_cap` | `operation_kind.body_depth_limit()` (P29-5) | done | they read the filler already |
| `tool_load/chipload.rs:609` | `RadialThinning::of_lut_role(ctx.pass_role)` | yes, 6a | the context carries the body facts for `Cut::of_sample`; `of_lut_role` equals `RadialThinning::of(family, role)` on every op that reaches the line (a drill short-circuits first, `chipload.rs:391`), so the read of `facts.thins_chip` is byte-identical |
| `tool_load/distribution.rs:780` | the same | yes, 6a | the histogram must read the gate's own rule |
| `deflection::exceeds_limit` (`tool_load/deflection.rs:138`) | `ctx.pass_role`, `ctx.radial_stock_to_leave_mm` | yes, 6a | `label_facts` fills `leave_after_mm` from the same two inputs (`compute/catalog/cuts.rs:425-430`); `Leave` maps one to one onto the four arms |
| Step 5, `feeds/calculator/feed.rs:102` | `RadialThinning::of(input.operation, input.pass_role)` | no, P29-7 | no gate reads `FeedsInput`; the fact keeps its label value until P29-7 gives it geometry; the 83 `FeedsInput {` literals are a P29-7 cost |

So the "about 140 test literals" splits into 38 `ToolpathLoadContext` literals (P29-6, section 7)
and 83 `FeedsInput` literals (P29-7). Counted with `rg "ToolpathLoadContext \{"` (43 hits, of
which 5 are destructure patterns in the gates) and `rg "FeedsInput \{"` (83 hits; 39 in
`feeds/tests.rs`).

### 1.4 Visible product surfaces

- The load card and the export report gain one row per non-body group (Entry, Cleanup, Link)
  per gate, each with its own population. The `entry_spike` / `entry_spikes` fields go
  (M3: the Entry group replaces them). This is a wire break of the MCP reply
  `get_tool_load_report` (one bless of the viz wire snapshot), stated in the commit.
- The headline verdict of each gate is the body group. The FM1 columns read the headline only.
- `UnmodeledReason::SteadyStateSamplesNotPresent` has no producer after 6b and is deleted (a
  wire break; MCP, the diagnostics adapter `from_tool_load.rs:1054`, `limits/check.rs`).
- The depth row prints the judged sample: its depth and its cap (6c, Q3).

## 2. The behaviour changes, each as the smallest edit

### 2.1 The kind group (6d; the type lands in 6a)

`limits/cut.rs`:

```rust
/// The group a gate reports a cut in (ruling R4).
pub enum CutGroup { Body, Entry, Cleanup, Link }

impl CutKind {
    pub const fn group(self) -> CutGroup {
        match self {
            Self::Body | Self::FirstPass(_) | Self::SpringPass | Self::CappedStep(_)
            | Self::SlotRing | Self::Drill => CutGroup::Body,
            Self::Entry(_) | Self::EntryApproach | Self::LeadIn => CutGroup::Entry,
            Self::Cleanup(_) | Self::DressupRelief => CutGroup::Cleanup,
            Self::Link | Self::LeadOut => CutGroup::Link,
        }
    }
}
```

One exhaustive match, no wildcard, no op name (limits invariant 1). `Drill` sits in the body
group: a drill op short-circuits every milling gate by its op kind before any sample is read
(rows `P28-04`, `P28-17`, `P28-20`, physical). `CutKind::is_judged_today` is deleted in 6d.

### 2.2 The gate context carries what `Cut::of_sample` needs (6a, byte-identical)

`ToolpathLoadContext` gains `body_facts: CutFacts` and `caps: &CapTable`, and loses the
fields that the facts replace for the gates (`pass_role` stays only where the vendor-row
lookup reads it: `matched_chip_envelope`, a row key, not a limit scope). The context is built by
one constructor (section 7). `LoadContextParts` owns the `CapTable`
(`CapTable::of_toolpath(&annotated.toolpath, result.semantic_trace.as_deref())`). The three
readers of section 1.3 read `ctx.body_facts`. No other reader changes. FM1 byte-equal.

### 2.3 The chip gate: the kind filter replaces the 95 % feed filter (6b)

`tool_load/chipload.rs:225-249` `steady_state_samples_for_toolpath`:

- delete `STEADY_STATE_FEED_FRACTION` and the `feed_threshold` test;
- the set is the samples of the toolpath that remove material (`removes_material()`);
- `SteadyStateSamples` and its name go (no "steady" filter is left): the function returns the
  removing samples and the flag `any_removing`. The entry and phantom split inside the loop
  stays until 6d.
- `UnmodeledReason::SteadyStateSamplesNotPresent` is deleted (no producer).

The P20e reference `ctx.steady_feed` stays for the commanded stage and the histogram (the
G-code F word); no filter reads it.

Measured with this edit (section 10.1): FM1 moves 2 cells.

### 2.4 The depth gate judges every sample (6c)

`tool_load/depth.rs:174-197` and `:226-282`. Today the gate reads the deepest steady sample and
solves the cap at that sample's width and chip; a sample with no width takes a full slot
(`:251-260`).

The edit:

1. The population is the body group of the samples that remove material and have a depth
   (`axial_engagement_mm > 0`). A sample with a depth and no width (no arc, `radial_woc_fraction`
   0) gives `NoNumber(NoMeasuredArc)`; the full-slot fallback goes. The population counts both.
2. Each sample with a width is judged: `depth <= cap(ae, fz)` with `cap` from
   `force_budget::rigidity_depth_cap` at the sample's own width and gate chip.
3. The verdict reports the sample with the largest `depth / cap` (the judged sample): its depth
   in `peak_mm`, its cap in `bound`. A new field `deepest_mm` keeps the deepest depth for the
   card text.
4. Cost: one forward read per sample, not one solve. Keep the current worst ratio `r*`. A
   sample can beat it only when `F(depth / r*) > budget` at its own width and chip (the force
   never falls with depth, P29-4 finding Q5: one root). Solve the cap only for those samples.
   The probe solved every sample: 1935 solves took 1.16 s in a debug build (FM1 Adaptive
   BullNose hardwood); the screen removes most of them. The GUI rebuilds the load report on its
   own thread (open defect of 2026-11-10), so the screen is required, not optional. When
   `depth / r*` is past the flute length, the screen reads the force at the flute length: a
   sample with no root below the flute (`NoRootBelowFlute`) cannot beat `r*`, and any root
   below the flute is shallower than `depth / r*`, so a force over the budget there sends the
   sample to the solve.

Why the depth ratio and not the force ratio: the two give the same state (depth over its cap
exactly when the force at that depth is over the budget), but not the same worst sample. On
FM1 Adaptive BullNose hardwood the worst force ratio (0.455) is at 1.795 mm with a cap of
4.9182 mm (depth ratio 0.365), and the worst depth ratio (0.3756) is at 1.7437 mm with a cap of
4.6419 mm; the deepest sample (1.8 mm) has 0.105. The gate is a depth gate, so it reports in
depth.

### 2.5 Every gate judges every group (6d)

One predicate in the place of three. `tool_load/locality.rs` loses `is_steady_state_for_gate`,
`is_phantom_transit` and `is_configured_entry`; it gains

```rust
/// The group of a sample that removes material; `None` for a sample that removes nothing.
pub fn judged_group(sample: &SimulationCutSample, ctx: &SampleContext<'_>) -> Option<CutGroup>
```

which reads `Cut::of_sample(..)?.kind.group()`. Each gate (`chipload`, `power`, `deflection`,
`depth`, `cut_force::cut_points`, `distribution`) folds its numbers into one accumulator per
group; the body accumulator gives the headline verdict as today; the others give
`ToolpathLoadVerdict::groups: Vec<GroupVerdict>`:

```rust
pub struct GroupVerdict {
    pub group: CutGroup,
    pub kinds: Vec<(CutKind, usize)>,   // the removing samples per kind
    pub chipload: ChiploadVerdict,
    pub power: PowerVerdict,
    pub deflection: DeflectionVerdict,
    pub depth: DepthVerdict,
    pub cut_force: Option<CutForceReading>,
}
```

The precedent is `material_split` (S6): same gates, same converters, the group named in the
message (`diagnostics/adapters/from_tool_load.rs:96-135`). Under the S6 split the groups are
built on the stock population only; an `OwnMaterialVerdict` keeps its four gates on the body
group, as `cut_force` today joins no split (`cut_force.rs:24-30`). A group with no removing sample is
absent. The diagnostics adapter emits a group diagnostic only when the group `Exceeds` (Q5);
a `Within` group is a row of the card, not a new `Info` diagnostic.

The same commit moves the other readers of the old predicates:

- `tool_load/mod.rs:296-309` (`chip_target_for_toolpath`, M8): the body group. Byte-identical in
  output: the row key `lut_key_diameter_for_cutter` ignores the depth (`feeds/geometry.rs:520-521`,
  `let _ = ap`).
- `stock/sim_triage.rs:944` and `:1714` (M8): the body group. On the four fixtures the flag-only
  filter and the body group select the same samples (reasoning from the sentry claim 2; not run).
- `session/compute.rs:2270` (the modulator's P20g sample table): every group (section 2.6).
- `compute/simulate.rs:1116` (`envelope_placement`): the body group only, marker `P28-21`
  narrowed (Q4).
- `compute/simulate.rs:2240`: the marker `C-LeadInApproach` goes; the stamper still writes the
  flag, and no gate filters on it.

### 2.6 The modulator reads the samples that the gates judge (6d and 6e)

6d, `session/compute.rs:2270`: the phantom skip goes; every removing sample of the move enters
`sample_engagements`, so a cleanup or a link move takes the minimum load cap over its own
samples (P20g) in place of the mean engagement. The minimum over the samples can be above or
below the cap of the mean engagement, so the feed of such a move can rise or fall; it stays
under the chip cap and the cutting ceiling. Rows `P28-22`, `P28-23` and `M2` lose their
modulator markers in the same commit, so the rows close whole.

6e, `dressup/feed_modulation.rs:679-708` `should_skip_modulation`: `LeadIn` and `LeadOut`
leave the skip list, under one rule: a lead move takes `min(stated lead feed, load caps of its
samples)`. It is never raised (F-040: the lead feed is the operator's). The chip cap and the
`ConstrainedMax` raise do not apply to a lead. `EntryPlunge` stays skipped (row `P28-30`, no
plunge model, P29-8). Row `P28-29` closes.

## 3. The limit that judges each cut kind

After 6d. "Bound" is the number the gate compares; "source" is where the bound comes from.

| Kind | Group | Limits that judge it | Bound | Source of the bound |
|---|---|---|---|---|
| `Body`, `FirstPass(_)`, `SpringPass`, `CappedStep(_)`, `SlotRing` | Body (headline) | ChipMax, ChipMin (median), SpindlePower, ToolDeflection, ForceBudget (depth and force reading) | band max and min; rated power at the sample rpm; the leave of a rough pass else 0.2 mm; the budget depth at the sample's width and chip | vendor row (`chip_envelope_row`); `MachineProfile::power_at_rpm`; `deflection::exceeds_limit` on `body_facts.leave_after_mm`; `machine.rigidity.depth_cap_of(body_depth_limit())` (35.3224 N, `hobby_budget`) |
| `Entry(Ramp)`, `Entry(Helix)`, `LeadIn` | Entry | the same five, on the sample's side engagement | the same bounds | the same sources |
| `Entry(Plunge)`, `Entry(PeckLadder)` | Entry | every limit gives `NoNumber(NoMeasuredArc)` | none | no axial or inclined model (P29-8) |
| `EntryApproach` | Entry | the same five; in practice NoNumber (no removing approach sample on the fixtures has a measured arc: shelf 23, Pocket 2) | the same | the same |
| `Cleanup(_)`, `DressupRelief` | Cleanup | the same five | the same | the same |
| `Link`, `LeadOut` | Link | the same five | the same | the same |
| `Drill` | Body | the drill gates only (`drill_gates.rs`); the milling gates return `NotApplicableForOp` by op kind | drill envelope | rows `P28-04`, `P28-17`, `P28-20` (physical) |

Two notes. The deflection bound of every group is the body cut's (`body_facts.leave_after_mm`):
an entry or a cleanup of a rough op leaves the same wall stock. The chip band of every group is
the body row: the vendor row has no entry or cleanup row; P29-8 owns the entry rules.

## 4. Commits

Five commits. Each passes the gate set alone. 6a moves nothing.

| Commit | Content | Moves | Rows closed |
|---|---|---|---|
| 6a | `CutGroup`; the context constructor (section 7); `body_facts` and `caps` on the context; the three readers of 1.3 read the facts | nothing (FM1 byte-equal, every pin stays) | none |
| 6b | the chip gate reads every removing sample (2.3) | 2 FM1 cells (measured) | `M1` |
| 6c | the depth gate judges every sample (2.4) | 20 FM1 cells (emulated) | `M5` |
| 6d | `judged_group`; group verdicts; `entry_spike` goes; M8 readers; the modulator sample table (2.5, 2.6) | 0 FM1 cells: the modulator half measured with a throwaway edit, the gate half emulated (under Q5); new group rows on every simulated op with entries, cleanup or links | `P28-22`, `P28-23` (Q1), `M2`, `M3`, `M8`, `Q10-LinkingInEntrySpan`, `Q10-EntryPlungeInNoSpan`, `Q4-SlotRingInferred`, `Q4-CappedStepNoTrace`, `C-LeadInApproach`; `P28-21` narrows |
| 6e | the modulator caps lead moves, never raises them (2.6) | 0 FM1 cells (reasoned) | `P28-29` |

Gates of every commit: fmt check; the named sentries `the_exemption_list_only_shrinks_p29_3`,
`every_limit_is_a_limit_id_p29_2`, `a_sample_states_its_cut_kind_p29_3`,
`every_op_declares_its_cuts_p29_3`, `gate_population_vacuity_xvac`,
`histogram_population_is_the_gate_population_g_cuthist`,
`one_steady_feed_for_the_cap_the_gates_and_the_card_p20e`, `every_gate_reads_one_feed_p16`,
`the_depth_that_cut_is_a_measured_load_g_s3depth`, `the_cut_force_is_the_deflection_force_p17`,
`a_rough_pass_deflects_up_to_its_stock_to_leave_p17`, `predicted_feed_gates_f035`,
`chipload_boundary_g_chip_ulp`, `a_criterion_carries_its_own_bound_g_s4bound`,
`a_weak_bound_cannot_refuse_an_export_g_s4weak`, `an_unmodelled_refusal_names_every_row_g_t19`,
`chipload_abstention_cannot_supersede_g_chipgate`, `arcfit_gate_population_d4`,
`a_move_holds_its_worst_sample_p20g`, `constrained_max_modulation_f039`,
`lead_in_out_feed_rates_f040`, `the_session_modulator_binds_on_the_force_budget_p16`,
`the_envelope_finding_reads_the_gate_feed_p20i`, `a_job_with_each_finding_exports_and_names_it_k6`;
`cargo test -p rs_cam_core -q --lib`; FM1 (`--features heavy-tests`); full clippy; viz `--lib`
and the viz tests `mcp_wire_surface_pin`, `the_limit_rows_read_their_own_bound_g_ownbound`,
`p17_visual_snapshots`, `the_sim_chart_seeks_on_a_dot_p17` (no viz test while
`pgrep -ax rs_cam_gui` prints).

New sentry, 6d: `tests/no_removing_sample_is_outside_the_gates_p29_6.rs`. Claims, on the four
fixtures of the kind sentry plus one helix fixture (Q6):

1. Every sample with `removes_material()` lands in exactly one group of exactly one gate
   population per gate, with a number or a `NoNumber` reason; the sum over the groups equals the
   removing count (no sample outside).
2. A sample that removes nothing is in no population.
3. The body group of each gate equals the headline population.
4. Non-vacuity: each fixture has removing samples in Entry; the shelf in Cleanup and Link; the
   Profile has `Entry(Plunge)` samples with `NoNumber`.

Inject the defect once (drop the Cleanup group from one gate) and confirm red.

## 5. Exemption rows and pins

Rows that close, per commit, and the pins of `tests/the_exemption_list_only_shrinks_p29_3.rs`
(today `ROWS` 63, `PHYSICAL` 8, `RULED` 3, `MAX_NEEDS_RULING` 52, `ROWS_WITH_A_FILE` 55; every
closing row is `NeedsRuling` and names a file):

| After | Rows closed | `ROWS` | `MAX_NEEDS_RULING` | `ROWS_WITH_A_FILE` | `CLOSED` gains |
|---|---|---|---|---|---|
| 6a | none | 63 | 52 | 55 | - |
| 6b | `M1` | 62 | 51 | 54 | `("M1", "P29-6")` |
| 6c | `M5` | 61 | 50 | 53 | `("M5", "P29-6")` |
| 6d (Q1: judge the Link group) | `P28-22`, `P28-23`, `M2`, `M3`, `M8`, `Q10-LinkingInEntrySpan`, `Q10-EntryPlungeInNoSpan`, `Q4-SlotRingInferred`, `Q4-CappedStepNoTrace`, `C-LeadInApproach` | 51 | 40 | 43 | `P28-22`, `P28-23`, `M2`, `M3`, `M8` with "P29-6" |
| 6d (Q1: keep the Link group out) | the same less `P28-23` | 52 | 41 | 44 | the same less `P28-23` |
| 6e | `P28-29` | 50 (51) | 39 (40) | 42 (43) | `("P28-29", "P29-6")` |

`PHYSICAL` 8 and `RULED` 3 stay. The `EXEMPTIONS` array length equals `ROWS`.

Rows that stay and change (text only, no pin):

- `P28-21`: narrowed to "the envelope finding places the body group only"; `where_`
  `src/compute/simulate.rs`; `closes_in` "P29-8 (entries get their own envelope)". It loses the
  ten gate files.
- `M4`: `closes_in` "P29-7 (a probe on ball and V tools)". `U-ResidueCleanup`: "P29-7".
- `P28-30` (plunge in the modulator) and `P28-30p` stay.

Markers that go: every `EXEMPT(P28-21, P28-22, P28-23)` in `tool_load/{depth,power,cut_force,
chipload,distribution}.rs` and `compute/simulate.rs` (that one becomes `EXEMPT(P28-21)`);
`EXEMPT(P28-22, P28-23)` and `EXEMPT(P28-21, M3)` in `power.rs`, `deflection.rs`, `chipload.rs`;
`EXEMPT(M1)` at `chipload.rs:230`; `EXEMPT(M5)` at `depth.rs:171`; `EXEMPT(M8)` in
`sim_triage.rs` and `tool_load/mod.rs:303`; `EXEMPT(M2, P28-22, P28-23)` at
`session/compute.rs:2270`; `EXEMPT(M2)` at `feed_modulation.rs:679`; `EXEMPT(P28-29)` at
`feed_modulation.rs:699`; the `Q10`, `Q4`, `P28-21..23` markers of `limits/sample.rs` and
`locality.rs` (the predicates are deleted); `EXEMPT(C-LeadInApproach)` at
`compute/simulate.rs:2240`.

## 6. Tests that go red

Found by `rg` over core and viz for the three predicates, `STEADY_STATE_FEED_FRACTION`,
`steady_state_samples_for_toolpath`, `EntrySpike`, `entry_spike`, `SteadyStateSamplesNotPresent`,
`is_judged_today`, and by the probe. A red outside this list in the gate set is a mismatch.

**6a (compile only, the constructor):** the 38 context literals (section 7): `src/tool_load/
{mod,depth,power,deflection,chipload}.rs` lib tests and 30 integration files (list: `rg -l
"ToolpathLoadContext \{" crates/rs_cam_core/tests`). No claim changes.

**6b:**

- `tests/one_steady_feed_for_the_cap_the_gates_and_the_card_p20e.rs`: `is_steady_abstain`
  (line 167) pins `SteadyStateSamplesNotPresent` on a capped op. New claim: the gate grades the
  chip of every removing sample at any feed, and the commanded stage states the steady feed.
- `tests/every_gate_reads_one_feed_p16.rs`: line 73 names `steady_state_samples_for_toolpath`
  as a scanned function. The scan entry moves to the renamed function.
- `tests/chipload_abstention_cannot_supersede_g_chipgate.rs:139` and
  `tests/every_limit_is_a_limit_id_p29_2.rs:320` list `SteadyStateSamplesNotPresent`: delete the
  member from the lists (compile).
- `tests/histogram_population_is_the_gate_population_g_cuthist.rs` claim 2 (doc line 33, "skips
  `steady_state_samples_for_toolpath`"): the histogram reads the new set; the claim stays.
- `src/tool_load/chipload.rs` lib tests that build a low-feed sample and expect the abstention
  (by `rg` in the file's test module; the implementer lists them).
- FM1 sim golden: 2 cells (section 10.1).

**6c:**

- `tests/the_depth_that_cut_is_a_measured_load_g_s3depth.rs`: the claims that the cap is solved
  "at the peak sample's arc width and chip" (folder doc) and any test that pins the full-slot
  fallback for a sample with no width. New claim: the verdict names the sample with the largest
  depth over its own cap; a sample with no width is `NoNumber`.
- `src/tool_load/depth.rs` lib tests at lines ~475 to ~779 that read the peak sample.
- `tests/gate_population_vacuity_xvac.rs`: the depth population counts change (no air samples).
- FM1 sim golden: 20 cells.

**6d:**

- `tests/a_sample_states_its_cut_kind_p29_3.rs`: claim 2 compares with the deleted predicate;
  claim 1 reads `is_air`. New claims: the group of every sample follows `CutKind::group`;
  `Cut::of_sample` is `None` exactly when the sample removes nothing.
- `tests/gate_population_vacuity_xvac.rs`: the fixture marks samples `in_transit_span` with no
  spans and no intent (line ~102); today they are phantom, after they are `Body` samples (no
  intent gives the body kind). New fixture: the vacuous trace is samples that remove nothing.
- `tests/arcfit_gate_population_d4.rs` `dogbone_bridge_samples_stay_out_of_the_gate_population`:
  after 6d a dogbone sample is in the Cleanup group. New claim: it is out of the BODY population
  and in the Cleanup group.
- `tests/an_unmodelled_refusal_names_every_row_g_t19.rs`, `a_criterion_carries_its_own_bound_g_s4bound.rs`,
  `chipload_advisory_disclosure_h4.rs`, `every_limit_is_a_limit_id_p29_2.rs`: verdict literals
  with `entry_spike(s)` (compile; the field goes, `groups` arrives).
- `tests/a_geodesic_arc_end_reads_the_stock_p22f2.rs:266` and
  `tests/every_wide_step_has_a_named_source_p22a.rs` import `is_phantom_transit`: they move to
  `judged_group` and filter `CutGroup::Body` (the same population on their fixtures, by the
  sentry claim 2).
- `tests/adaptive3d_interior_cell_parity_f029.rs`, `adaptive3d_planner_stock_xy_f027.rs`,
  `axial_engagement_vs_dpp_detector_f2.rs`, `adaptive_feed_modulation_pipeline_f036b.rs`: doc
  references to the predicate; at risk if they compute a population through it (not checked by
  a run; slow 3D sims, outside the gate set).
- `src/tool_load/locality.rs` tests of the three predicates: deleted with them; the
  `classify_sample_locality` tests stay.
- `src/gcode/mod.rs:2706-2859` and `src/tool_load/verdict.rs` test literals with `entry_spike`.
- viz: `rs_cam_viz/tests/the_inspector_nests_once_dc5.rs` (names an entry spike), the MCP wire
  snapshot `mcp_wire_surface_pin` (the `entry_spike` keys go, `groups` arrives: one bless, a
  stated break), any viz card snapshot of a load row.
- `tests/a_move_holds_its_worst_sample_p20g.rs`: if it pins that a phantom sample stays out of
  the modulator table, it goes red by design (new claim: every removing sample of a move binds).

**6e:** `tests/lead_in_out_feed_rates_f040.rs` (it pins that a lead feed is not modulated). New
claim: a lead keeps its stated feed unless a load cap of its own samples is lower; it is never
raised.

## 7. The context literals: a constructor that the compiler holds

The problem: `ToolpathLoadContext` has 38 struct literals (6 in `src/tool_load/`, 1 in
`session/compute/toolpath_diagnostics.rs` through `LoadContextParts`, the rest in tests). Each
states `operation_family`, `pass_role` and `operation_kind` by hand, so a test can state a
Pocket family with a Drill kind (`tool_load/mod.rs:806-825` does). A new required field with no
default makes every literal state the facts by hand too, and nothing ties them to the op.

The rule: **the facts of the context come from the operation, and only from it.**

1. Make the fields of `ToolpathLoadContext` private (no `pub`). The gates live in child modules
   of `tool_load`, so they still read the fields; no code outside `tool_load` can build a
   literal (E0451).
2. One constructor:

   ```rust
   impl<'a> ToolpathLoadContext<'a> {
       pub fn of_operation(
           toolpath_id: ToolpathId,
           operation: &OperationConfig,
           tool: &'a ToolDefinition,
           material: &'a Material,
           steady_feed: SteadyFeedMmMin,
           spans: Option<&'a [Span]>,
           caps: &'a CapTable,
           drill_op: Option<&'a DrillOp>,
       ) -> Option<Self>
   }
   ```

   It fills `operation_kind`, the vendor lookup family and role (`feeds_style`), `body_facts`
   (`operation.cuts().body_facts()`), `commanded_thinning_lift` and the leave from the
   operation. `None` when the op states no feed (S11, as `load_context_parts` does today).
   `LoadContextParts` keeps the owned parts and calls it.
3. Accessors for the few fields that other modules read (`toolpath_id()`, `tool()`,
   `material()`, `operation_kind()`, `spans()`), and one `with_material(&Material)` for the S6
   own-material copy in `evaluate_toolpath` (`tool_load/mod.rs:667-673`, today a struct update).
4. Tests call `of_operation` with `OperationConfig::new_default(kind)` (`compute/catalog.rs:510`)
   or `compute::fixture_values::fixture_op(kind)` (`test-support`), and `CapTable::empty()`
   (new, a table with no capped move) for a hand-built trace. A test that wants a context
   that no op gives (the Pocket-family Drill of `mod.rs:806`) states the real op instead; that
   test exists to prove the drill short-circuit, which reads only `operation_kind`.

The compiler holds the rule: outside `tool_load` no literal compiles; inside, the fields have
one filler.

Is 6a compile only? A scan of the 39 literal sites (a script over core and viz):

- 3 state a triple that no op gives: `tool_load/mod.rs:811` (Pocket family, Roughing, kind
  Drill), `tests/the_simulation_is_placed_not_fed_back_k7.rs:338` (Pocket family, Finish role),
  `tests/arcfit_gate_population_d4.rs:526` (Contour family, Finish role, kind Profile; Profile
  is Roughing). Each moves to the op it means.
- 10 take the family, the role or the kind from a variable (for example
  `the_depth_that_cut_is_a_measured_load_g_s3depth.rs:243`: Pocket family with any `op`;
  `finish_depth_is_reported_not_capped_fm6.rs:238`; `a_vbit_row_is_read_at_its_printed_key_b4.rs:303`);
  the implementer checks each pair.
- Every test literal sets `commanded_thinning_lift: None` and `radial_stock_to_leave_mm: None`.
  Under the constructor they take the op's values: the lift stays `None` when the op states no
  stepover or no depth (S11 defaults); the deflection limit moves only when the op's leave is
  above 0 (`Leave::Mm`; `NoField` and `Zero` both give 0.2 mm). A test whose fixture op states a
  leave or both stepover and depth can change its expectation. So 6a is byte-identical in the
  product and compile-only in tests only where the scan shows no such op; the implementer
  reports the count of test expectations that move, and each is a red to name, not to bless. The sentry `every_op_declares_its_cuts_p29_3` claim 3 already holds
`cuts().body_facts().depth_rule == op_type().body_depth_limit()`.

## 8. The operator's concern (2026-11-12): what the simulation shows after modulation

"simulation should run whats going to happen in real life. having a post-sim change seems not
ideal". Facts from the code; no fix is designed here.

1. **Order.** `run_simulation` simulates, then calls `modulate_simulation_trace`
   (`session/compute/simulation.rs:349`), which runs only with `adaptive_feed_modulation`
   (`:639`). The GUI and MCP path calls the same door after `AdoptSimulation`
   (`session/command.rs:2863`).
2. **The export.** The modulator writes the modulated toolpath into the result slot
   (`simulation.rs:856-861`, provenance `ResultProvenance::Modulated`), so the export writes the
   lowered feeds. The next simulation simulates the modulated toolpath (the request takes
   `result.annotated()`, `simulation.rs:157`), so its samples carry the lowered feeds as their
   own `feed_rate_mm_min`. Inference, not shown: this explains why P29-0 found the 95 % filter
   dropping 91 % of operator idx 1 (a feed cap or `feedopt` also lowers a sample feed).
3. **The load cards.** The samples keep the first run's geometry, feed and time
   (`feed_rate_mm_min`, `chipload_mm_per_tooth`, `segment_time_s`, `mrr_mm3_s`). The modulator
   writes each modulated feed into `trace.predicted_feeds` (`simulation.rs:1036-1038`) and, with a
   kinematics block and `use_predicted_feed_in_gates`, lowers it to the reached feed
   (`:1046-1070`). Every gate reads `effective_feed_for_sample` (`tool_load/mod.rs:77-88`), so
   power, deflection, chip, depth cap and force cards show the LOWERED feed on the first run's
   engagement. Geometry does not depend on feed, so the engagement is still true; the feed
   readers are right after one modulation. Readers of the first-run feed or time remain, for
   example the chip gate's 95 % filter (`chipload.rs:242`, `s.feed_rate_mm_min`), the sample
   chip `chipload_mm_per_tooth` and the force card weight `CutPoint3::weight_s`
   (`segment_time_s`); this is not a census.
4. **The time estimate.** The cycle time is re-integrated on the modulated feeds only when the
   machine has a kinematics block (`simulation.rs:902`, ruling N7). Without one, the trace keeps
   the first run's runtime at the commanded feeds, while the export writes the lowered feeds.
   The operator file `planning/rivmap_block_job/nz_wanaka_21_operator.toml` holds a
   `[job.machine.kinematics]` block (line 77, read-only grep), so its time is re-integrated.
5. **What P29-6 changes for it.** 6b deletes one first-run-feed reader (the 95 % filter). 6d and
   6e give the modulator more samples: cleanup and link moves take the minimum load cap over
   their own samples (today the mean engagement), and lead moves can be lowered. So more moves
   change feed after the simulation (mostly lower: a lead never rises), and the exported feed
   of a move over a limit falls. On FM1 no feed moves (section 10.3, measured for 6d). On wanaka, from `P29-0_probes.md`
   section 7.3 (forces at the engaged depth `axial_engagement_mm`, as the gates read it): work idx 1 cleanup has 286 samples over the budget (90.3 N peak), operator
   idx 14 cleanup 1 212, operator idx 14 link bridges 235; those moves would be lowered by the
   force-budget cap with modulation on. This widens the post-simulation change the operator
   questions; the later design step should take it as input.

## 9. Budget, placement, and export

**Findings that depend on the budget value (35.3224 N, the largest printed hobby SLOT force).**
The depth gate (6c) and the force reading of every group read it. On the fixtures and FM1 every
judged sample is under it (worst force ratio 0.735, FM1 Adaptive3d hardwood; worst depth ratio
0.7206, same cell), so no finding of this step depends on its value there. On wanaka the
findings that appear depend on it entirely: the cleanup, link and capped-step counts over the
budget of `P29-0_probes.md` sections 7.3, 7.4 and 8 (for example 59 809 of 876 293 samples of
work idx 1). For a partial-width cut the budget is probably low (hand-over, "P29-4 status";
the Sienci pocketing row 5.6 mm at 2.85 mm width is not checked against the model). So the new
Cleanup, Link and Entry force and depth findings on wanaka are advisories (`MachineForceBudget`
does not gate) that a later budget tune can move with no code change. The 6c change of the
judged sample moves only which sample is named, not the budget.

**The placement and the proposal (two chips; `propose` checks no Runtime floor).** P29-6 does
not touch `feeds/envelope.rs` `propose`, `placement_of`, `judged_ceiling` or the Runtime limit.
The envelope finding `envelope_placement` keeps the body group only and is not changed in
6b, 6c or 6e; in 6d only its predicate call changes (the same population, by the sentry claim
2). No pre-simulation number moves (FM1 matrix file: 0 cells in every commit, measured for 6b).

**Can a new finding refuse an export?** No. K6: the export refuses only on `EmitError` (inch
units, bad feed, controller compensation; `gcode/mod.rs:37-47`), which no load finding reaches.
A load row warns when it exceeds a bound whose `BoundSource::gates_export()` is true
(`tool_load/verdict.rs:1207-1209`, `CriterionStatus::warns_on_export` `:1341-1347`), and is an
advisory line otherwise (`gcode/mod.rs:744-786`). The new group rows reach the export through
`criteria()` the same way: a group power, chip or deflection exceedance is a warning row
(`MachinePowerCurve`, `VendorChipBand`, `DeflectionBudget` or `StockToLeave` gate), a group
depth or force exceedance is an advisory (`MachineForceBudget`). None refuses.

## 10. Predictions

Computed on HEAD `d84ca2e76`. Method:

- **Probe, no product edit** (every number of 10.2, every 6c and 6d number): two throwaway
  test files under `crates/rs_cam_core/tests/` (deleted) that include one probe module from the
  scratchpad. The probe rebuilds each gate's population from the gate's own public doors
  (`power::sample_power_kw`, `deflection::sample_tip_deflection_mm`,
  `display::peak_chip_per_tooth`, `cut_force::sample_peak_force_n`,
  `force_budget::rigidity_depth_cap`, `effective_feed_for_sample`) and the kind from
  `CutKind::of_sample` with the gates' span lookup.
- **Calibration**: the probe's "today" populations and peaks equal the real verdicts on every
  fixture and every FM1 cell (for example Pocket: chip 720 of 1017, power 1110, depth 1188,
  force peak 4.8449 N, all equal to `tool_load_report`; shelf: chip 4085 of 5046, power 4424,
  depth 6888).
- **Run with a throwaway edit** (6b, and the modulator half of 6d in 10.3): `steady_state_samples_for_toolpath` read
  `removes_material()` and dropped the feed test; FM1 ran without the update variable, so no
  fixture was written; the diff printed 2 cells. `chipload.rs` was restored from a byte copy,
  checked by `sha256sum -c`, and touched.
- **Reasoning only**: 6e, the M8 triage readers, the wanaka rows.

### 10.1 FM1

Matrix file (pre-simulation): 0 cells in every commit (measured for 6b; no other commit
changes a pre-simulation reader).

Simulation file (modulation on, `SimulationOptions::default()`):

| Commit | Rows | Cells | Columns | Basis |
|---|---|---|---|---|
| 6a | 0 | 0 | - | byte-identical by construction |
| 6b | 2 | 2 | `chipload_detail` (DropCutter EndMill softwood and hardwood: `SteadyStateSamplesNotPresent` -> `CutterModeUnsupported`) | run with the edit |
| 6c | 7 | 20 | `depth_cap_mm` 7, `depth_peak_mm` 4, `depth_cap_factor` 3, `depth_detail` 3, `diagnostic_ids_after_sim` 3 | probe (emulated) |
| 6d | 0 | 0 | - | modulator half: run with the edit; gate half: probe (no group exceeds; Q5) |
| 6e | 0 | 0 | - | reasoned (no lead sample on FM1 holds a binding cap) |

6b detail: every removing body sample of DropCutter EndMill is air class with an arc of 0, so
the vestigial chip-model test counts it as "chip geometry unsupported"
(`chipload.rs:694-700`, `:887-902`). The reason text is not true (Q7).

6c rows: Adaptive EndMill hardwood (cap 17.6593 -> 17.2475), Adaptive EndMill mdf, Adaptive
BullNose softwood and mdf (Reported -> a judged sample with a cap: 1.2656 / 17.9092,
1.7953 / 17.9184, 1.7953 / 17.2995; detail `Reported` -> empty; `load.depth.reported` ->
`load.depth.within`), Adaptive BullNose hardwood (1.8000 / 17.1952 -> 1.7437 / 4.6419),
Adaptive3d softwood and hardwood (cap 6.7487 -> 7.9231, 3.7314 -> 4.1629). The Adaptive3d cap
rises because today's deepest sample (sample 17) is air class with no width and takes the
full-slot fallback. The state column reads `Within` before and after on every row (a
`Reported` verdict prints `Within`). Under Q3's other choice (`peak_mm` stays the deepest), the
4 `depth_peak_mm` cells do not move.

Why 6b moves no state: the chip medians move little (shelf 0.0314 -> 0.0311; FM1 Adaptive
BullNose 1520 -> 1525 samples, median equal), and every band whose median is below its minimum
is a low-side advisory row today (a point, an extrapolated row or a row with no `ae` window).

### 10.2 The four fixtures of the kind sentry

Modulation off (the sentry's options). Full table: `P29-6_predictions.csv`, table `K` (per
kind and per group, per gate) and table `V` (headline verdicts per commit). The group labels of
the CSV map onto section 2.1: "Entry: entries" and "Entry: approach and lead-in" are the Entry
group, "Cleanup: cleanup" and "Cleanup: dressup relief" the Cleanup group.

Samples that each gate judges, today -> after (after = removing samples with a number; "nn" =
removing samples judged with no number):

| Fixture | Gate | Body today -> after | Entry after | Cleanup after | Link after |
|---|---|---|---|---|---|
| Pocket | chip | 720 -> 1110 (+390 slot ring), 94 nn | 297 (today on the advisory track: 0) | - | 0 removing |
| Pocket | power, deflection, force | 1110 -> 1110, 94 nn | 297 (advisory 297 today), 34 nn | - | 0 |
| Pocket | depth | 1188 -> 1110 (78 with a depth and no width) | 297 (20 no width) | - | 0 |
| Pocket + dogbones | power | 1104 -> 1104 | 297 | relief 24, 96 nn | 0 |
| Profile | chip, power, deflection, force | 791 -> 791, 114 nn | lead-in 21 (advisory 21 today), plunge 22 nn | - | lead-out 6, 8 nn |
| Profile | depth | 802 -> 791, 11 no width (Reported -> Within) | lead-in 21 (9 no width) | - | lead-out 6 (2 no width) |
| Shelf | chip | 4085 -> 4424 (+339 capped), 2665 nn | 461 (0 today), 190 + 23 nn | 918, 405 nn | 12, 110 nn |
| Shelf | power, deflection, force | 4424 -> 4424 | 461 (advisory 374 today) | 918 (0 today) | 12 (0 today) |
| Shelf | depth | 6888 -> 4866 (2022 no width) | 462 (95 no width) | 991 (274 no width) | 14 (70 no width) |

Every group verdict is `Within` on every fixture. Values worth naming: the shelf cleanup's peak
power 0.0827 kW is above the body's 0.0768 kW; its force 18.724 N against the body's 19.236 N.
The headline verdicts that move (table `V`): chip populations and peaks (6b), the Profile depth
Reported -> Within with cap 18.1714 mm (6c), the shelf depth cap 19.1205 -> 17.0971 mm at the
same 5.5 mm (6c).

The removal check (Q1): removed volume over `ap x ae x advance`, median per kind, once with
`ap = axial_doc_mm` (the power and chip depth) and once with `ap = axial_engagement_mm` (the
force, deflection and depth depth). Body 1.17 to 1.33 (both); Link 1.22 (shelf, both);
LeadOut 1.84 (Profile, both); Cleanup 2.48 (shelf, both); DressupRelief 0.40 (Pocket, both);
Entry(Ramp) 1.58 / 1.38 (Pocket), 1.00 / 0.95 (shelf). A reading of neighbour stock would give
a ratio well under 1. Only the dogbone relief does; the links, lead-outs and cleanup remove at
least the rectangle they read. The ratio is an instrument with a bias (body over 1), not a
proof, and wanaka was not checked.

Not held by any fixture: `Entry(Helix)`, `Entry(PeckLadder)`, `Drill`, `CappedStep(LevelContour)`,
`FirstPass`, `SpringPass` (the hand-over list). The new sentry adds one helix fixture (Q6).

### 10.3 6d and 6e on the modulator

6d, measured: a throwaway edit at `session/compute.rs:2270` (the phantom skip replaced by
`!sample.removes_material()`) left FM1 byte-equal, both files (the run rebuilt the crate;
the file was restored from a byte copy, checked by `sha256sum -c` and touched). The cleanup
samples on FM1 are light (Adaptive3d: 10 with a number, force up to 2.756 N, power up to
0.0019 kW), so no cap moves a feed. 6e, reasoned: no lead sample on FM1 has a number, so no lead
cap binds: 0 FM1 cells. The implementer reports the FM1 diff of each; a moved cell is a
mismatch.

### 10.4 Wanaka (reasoning from `P29-0_probes.md`; not simulated)

Measured there with modulation on, on older code (the Factor rule era for the Adaptive depth).
After 6d the operator file shows: an Entry group on idx 1, 2, 14, 15 (idx 2: 36 ramp samples over
the budget, peak 104.8 N; no entry over power or 200 um); a Cleanup group on idx 1, 2, 14
(idx 14: 1 212 samples over the budget); a Link group on idx 1, 14 (idx 1: 6 over, 136.8 N;
idx 14: 235 over). All are advisories. The chip gate judges the full body of idx 1 and idx 8
(today 8.6 % and 0 %); P29-0 section 4 predicts no chip verdict change. The counts depend on the
budget (section 9). The implementer does not simulate wanaka; the lead reads it at the next
operator simulation.

### 10.5 What stays byte-identical

- Every pre-simulation number (Suggest, envelope, FM1 matrix file), every toolpath, every
  G-code from a generation with no simulation after it.
- 6a: everything.
- The headline power, deflection and force readings on the four fixtures and FM1 (the body
  group equals today's span-filter population there: sentry claim 2 and the probe). Not proven
  for every op: `body_kind` (`limits/sample.rs:173-210`) gives `Body` to a cut-intent sample
  under a `LinkBridge`, `LeadOut` or `Entry` span, which today's filter drops. Rule after 6d:
  the intent decides, as in `CutKind::of_sample`, so such a sample is judged in the body
  group. The implementer reports that count on the shelf and on the FM1 cells (0 on the four
  fixtures by claim 2) before 6d; the lead reads it on wanaka at the next simulation.
- Not identical: the chip population (6b), the depth verdict's judged sample (6c), the load
  report wire shape (6d), the modulated feeds of cleanup, link and lead moves where a cap binds
  (6d, 6e).

## 11. Docs to change

- `crates/rs_cam_core/src/tool_load/CLAUDE.md`: the first invariant ("Every gate filters
  `in_transit_span` samples through `is_steady_state_for_gate`") becomes "every gate judges
  every removing sample in the group `locality::judged_group` gives; the headline is the body
  group"; the P11b2 line loses "95 %"; the depth sentry text (judged sample, no full-slot
  fallback).
- `crates/rs_cam_core/src/limits/CLAUDE.md`: `CutGroup`; `is_judged_today` gone; the open item
  "the gates keep the span filter until P29-6" closes; the Q4 label gap as an open item.
- `crates/rs_cam_core/src/dressup/CLAUDE.md`: P20g ("phantom samples dropped by
  `is_phantom_transit`") becomes "every removing sample of the move"; F-040 leads (6e).
- `crates/rs_cam_core/src/stock/CLAUDE.md` if it names the triage filter (M8).
- `FEATURE_CATALOG.md`: the load report rows per group; the chip gate's population; the depth
  gate's judged sample.
- `P29_FINDINGS.md` section 6, the P29-6 row: the scope of section 1 (capped steps were judged;
  the modulator half; `P28-21` narrowed; M4 and residue to P29-7).

## 12. Risks and questions for the lead

Most important first. Each question has my choice.

**Q1. Lead-outs, links and dogbone reliefs (row `P28-23`).** R4 names entries and cleanup only.
The row says the operator rules here. Evidence: the links and lead-outs remove at least the
rectangle they read (10.2); on wanaka the link bridges carry real force (operator idx 1:
136.8 N, idx 14: 235 samples over the budget). Choice: judge them, the dogbone relief in the
Cleanup group and the links and lead-outs in a Link group, as warnings like every cut; ask the
operator in one line before 6d. The other choice keeps `P28-23` (pins of the second 6d row).

**Q2. "Removes material" means `removes_material()`.** Choice: yes (1.2). An air-class removing
sample is judged with no number when it has no engagement. The other choice (`!is_air()`)
leaves 94 to 45 551 removing samples per op outside, against the acceptance.

**Q3. What the depth row prints.** Choice: the judged sample's depth and cap, plus `deepest_mm`
for the card. The other choice keeps the deepest depth in `peak_mm` beside a cap of another
sample (4 FM1 cells fewer), which prints a comparison of two samples.

**Q4. The envelope finding.** It places samples against the body cut's envelope. An entry is a
different cut. Choice: body group only, `P28-21` narrowed to it and re-filed to P29-8. The other
choice places every group and gives false "outside the envelope" findings on entries.

**Q5. A Within group gives no diagnostic.** Choice: a group diagnostic only on `Exceeds`; the
card shows every group. The other choice (the S6 pattern: one `Info` per gate per group) moves
`diagnostic_ids_after_sim` on every FM1 cell with entries (all 2D cells and Adaptive3d) and
needs a new prediction run.

**Q6. A helix fixture.** The sentry holds no `Entry(Helix)` sample. Choice: add the 2D Adaptive
default helix entry on the outline (the Adaptive default is Helix) to the new sentry.

**Q7. The chip reason on a sample with arc 0.** 6b measured `CutterModeUnsupported` on
DropCutter EndMill. The text ("chip geometry unsupported") is not true: the samples are skins
with no measured arc. Choice: count an arc of 0 as no arc in the same commit, so the reason
reads `ArcEngagementNotCaptured`. That changes the 2 predicted cells to `ArcEngagementNotCaptured`;
the lead picks one before 6b, and the CSV holds the measured text.

**Q8. The relative entry-load rule (`project.entry_load`, `stock/sim_triage.rs:1290-1460`).**
R4 says it goes when the absolute rule exists. After 6d ramps, helices and lead-ins have an
absolute reading; plunges do not (P29-8). Choice: keep the rule until P29-8; a plunge is the
case it finds today (P29-0 section 7.2).

**Q9. ChipMin on the Entry group.** A ramp is fed slow by rule (the G6 ramp feed,
`feeds/ramp.rs`), so its chip sits low. No base band of this note has a hard low side (every
band with a minimum is a point, an extrapolated row or a row with no `ae` window), so nothing
could trip here. On an op whose row has a hard low side, 6d gives a `load.chipload.low`
warning on the Entry group whenever the entry median is under the band minimum. Choice: judge
it (every limit applies unless a physical reason exists; the operator decides an exemption),
and name the group in the message; the entry feed rule itself is P29-8. The other choice is
a new row (ChipMin, Entry group, closes P29-8), which raises a pin.

**Q10. 6e at all.** The lead modulation touches operator-tuned feeds. Choice: lower only, never
raise (2.6). The other choice re-files `P28-29` to P29-8 with the entry rules.

Risks:

- The GUI thread rebuilds the load report (open defect 2026-11-10). 6c without the screen of
  2.4 multiplies that time.
- Group verdicts add one population walk per gate per group; the walk is shared (one pass, one
  accumulator per group), so the cost is the per-sample `Cut::of_sample` (one cap lookup, one
  span walk), measured nowhere yet. The implementer reports the load report build time on the
  shelf before and after.
- 6d deletes three public predicates that tests outside the gate set import (section 6); a slow
  integration test may go red by compile only.

## 13. Findings outside the scope

- **F1.** The depth gate's full-slot fallback (`depth.rs:251-260`) names an air-class sample with
  no width as the peak on FM1 Adaptive3d, DropCutter EndMill and other cells; the cap it prints
  is a slot cap for a skin.
- **F2.** `CutterModeUnsupported` text on samples with arc 0 (Q7); the chip loop's
  "vestigial" validity test (`chipload.rs:678-701`) is the cause.
- **F3.** `rs_cam_viz/src/ui/sim_op_list.rs:1187` cites `CriterionStatus::refuses_export`; the
  method is `warns_on_export` (K6). A stale doc.
- **F4.** The hand-over's WAIT list is stale on three readers (1.1 item 5).
- **F5.** `lut_key_diameter_for_cutter` ignores its depth argument (`let _ = ap`), so every
  "peak depth for the row key" walk (`chipload.rs:458-463`, `tool_load/mod.rs:296-309`) computes
  a value no reader uses.
- **F6.** The depth predicate and the force reading are one predicate after 6c (same engagement
  and chip on flat and bull tools). P29-10 can merge the two rows.
- **F7** (probe hygiene, as P29-5 F5): restore with `cp` plus `touch`, then check the hash; a
  `cp -p` keeps the old mtime and cargo keeps the edited build.
