# Adversarial review — the five-limit surface

Reviewed 2026-09-17. Read only. The subject is
`planning/load_model_2026-09-16/WHERE_THIS_LANDS.md` §4 and the prototype in
`planning/load_model_2026-09-16/derate_bench.html:933-1180`.

**Citation form.** Another session moves modules under
`crates/rs_cam_core/src/` as this review is written. Every code claim below
names the **symbol** first and the path and line second. The symbol is the
durable half. `planning/structure_2026-09-17/FEEDS_WAVE.md:42-44` gives the
same instruction to its own readers: "Re-locate each item by symbol name, not
by line."

The design commits to four rules:

1. Every limit draws identically. No badge marks one as better evidenced.
2. A limit that is not set draws no bar and never appears comfortable.
3. The source of a limit lives behind a hover and names the setting that
   moves it.
4. The scale is shared between the two stages, so the operator can compare
   the prediction to the run.

Rules 2 and 3 already ship. Rule 1 reverses a dated decision. Rule 4 is
wrong.

---

## 1. Findings that would sink it

### S1 — This surface already exists, and rule 1 deletes part of it

`verdict_badge` (`crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1151` at time
of survey) draws one row per limit today. It reads `CriterionStatus`
(`crates/rs_cam_core/src/tool_load/verdict.rs:583`) and renders:

- **a percent of the limit** — `pct_of_cap`
  (`crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1142`), which is the design's
  "78 % of its closest limit";
- **an unset state that is neither pass nor fail** — the `is_vacuous` branch
  paints `∅` in dim text (`verdict_badge`,
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1167-1172`). That is the
  design's rule 2, shipped, with its reason dated in the code: a gate handed
  an empty population returned `Within` and painted a green "0 %",
  indistinguishable from a measured clean cut;
- **the source behind a hover** — `verdict_tooltip`
  (`crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1197`), which pulls the
  wording from core through `CriterionStatus::vacuity_clause`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:622`) so the GUI, the CLI and
  MCP cannot word it differently. That is rule 3;
- **a confidence mark** — `Confidence::Approximate` gets `WARNING_MILD` and a
  `≈` suffix (`verdict_badge`,
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1181-1184`).
  `criterion_detail` (`crates/rs_cam_viz/src/ui/sim_op_list.rs:1179`) prints
  the same distinction in prose.

So the design's rules 2 and 3 are a restatement of shipped behaviour, and
rule 1 is a request to **remove** the fourth item. The type it would remove
carries an explicit instruction: `Confidence`
(`crates/rs_cam_core/src/tool_load/verdict.rs:205`) states that the UI must
render `Approximate` differently from `Validated` so users do not anchor on
it.

Provenance also changes behaviour today, not only appearance.
`ChipBoundsSource::low_side_is_advisory`
(`crates/rs_cam_core/src/tool_load/verdict.rs:852`) downgrades a burn-side
trip to an advisory when the bound is weakly provenanced, and keeps the
breakage side hard for every source.

The operator asked for uniform treatment and rejected confidence badges. The
codebase has a badge, it is called a badge, and it has a dated reason. The
operator may still want it gone. They should decide that knowing it is a
removal, not a new-surface choice.

**Smallest correction.** State plainly that rule 1 retires
`Confidence`-dependent rendering in `verdict_badge`, and get that ruling
before building. §2 W1 gives a form that satisfies both constraints.

### S2 — The shared scale is not shared, and the two stages do not hold the same quantity

Two faults sit on top of each other.

**The prototype does not do what it claims.** In `renderPrototype` the
pre-stage bar fills the row element to `f × 100 %`, where the full element
means the limit (`derate_bench.html:1113`). The post-stage histogram spans
`rowAxisMax` (`derate_bench.html:1067-1070`), which returns
`max(limit, value × 1.45) × 1.3` — never less than 1.3 times the limit. The
limit marker therefore sits at 77 % of the width or less
(`renderPrototype`, `derate_bench.html:1109-1110`). The same row means "limit
at the right edge" before the run and "limit at three quarters" after it. The
comment at `derate_bench.html:1105-1106` states the opposite of the code
beneath it.

That fault is small. The fault under it is not.

**The prediction and the samples are different quantities on two of the five
rows.** The repository established this and built a type to stop it. The
module header of `feed_explanation`
(`crates/rs_cam_core/src/feeds/feed_explanation.rs:12-30`) records a live
session that produced four chipload numbers for one operation, no two
agreeing, with nothing on screen saying they were four different quantities.
The gate observes `commanded_fpt × predicted_feed / commanded_feed`
(`crates/rs_cam_core/src/feeds/feed_explanation.rs:47-49`). Suggest's number
is the commanded value. The two differ by the achieved-feed ratio on every
correct run.

The power row is worse. `WHERE_THIS_LANDS.md:38-47` records that the
recommendation computes `power_kw` at the calculator's depth, before
`clamp_dpp_to_rigidity`
(`crates/rs_cam_core/src/feeds/suggest.rs:2370`) cuts that depth by about 3.5
times. A shared axis draws that 3.5× gap as a prediction the run
contradicted. The operator learns nothing true. The producer is wrong, not
the machine.

**What the design says when the two disagree: nothing.** It shows two
pictures and a sub-line claiming the share past the limit is what matters.

**Smallest correction.** Drop rule 4. Each stage states its own quantity by
name, and the post stage names the order statistic it used.
`ObservedStatistic` (`crates/rs_cam_core/src/feeds/feed_explanation.rs:76`)
already carries `Median` and `Peak` with operator-facing labels, and it
exists because "chipload" alone does not say which is on screen. It has no
caller in `rs_cam_viz` today. Then fix the power producer —
`WHERE_THIS_LANDS.md:205` step 2 — before any comparison ships.

### S3 — "Share of the run past the limit" is a sample count, and it is the wrong statistic for two of the five metrics

`renderPrototype` computes the share as
`samples.filter(...).length / samples.length` (`derate_bench.html:1133`).
That is a count of samples, not a share of the run.
`SimulationCutSample::segment_time_s`
(`crates/rs_cam_core/src/stock/simulation_cut.rs:169`) carries the time
weight and the prototype ignores it. A slow deep corner and a fast shallow
pass contribute equally. The foot text at `derate_bench.html:1153` asks "is
this one corner or half the job?", which is a time question, and answers it
with a count.

The deeper fault is that duration does not matter for every limit.

- **Gantry push.** The drive loses position once. Every coordinate after that
  is wrong and the part is scrap. A 0.02 % share is a total loss.
- **Tool deflection.** Past the fracture load the tool breaks. Nothing breaks
  for 0.3 % of the run.
- **Chip thickness, a floor.** Rubbing is thermal. Duration is exactly what
  matters.
- **Spindle power.** The motor's thermal mass absorbs a short excursion.
  Duration matters.

The core already made this distinction, and made it per side.
`ChiploadStatistic` (`crates/rs_cam_core/src/tool_load/verdict.rs:813`) uses
the **median** of in-cut samples on the burn side and the **per-sample peak**
on the breakage side. The design overwrites a reasoned, shipped choice with
one statistic for all five rows.

A brief catastrophic excursion is also invisible in the drawing.
`drawHistogram` clamps any sample above the axis into the last bin
(`derate_bench.html:1047`), so a sample at five times the limit draws in the
same column as one at 1.9 times.

**Smallest correction.** Each row declares its own headline statistic and
prints its name. Time-integrated metrics report a time-weighted share.
Instantaneous-failure metrics report the peak and the count of excursions,
never a share. Keep the histogram as the picture; change the number above it.

### S4 — The surface claims the page-one position and does not read triage

`crates/rs_cam_viz/CLAUDE.md` states that simulation presentation reads
bounded triage first, and that `NotMeasured` is an abstention, not a healthy
zero. `crates/rs_cam_core/CLAUDE.md` states that
`ProjectSession::simulation_triage` is read before raw issue counts.
`SimulationTriage` (`crates/rs_cam_core/src/stock/sim_triage.rs:217`) calls
itself the page-one answer, with `measurability` read first, then class A
safety, then class B actions.

The post-stage screen shows five distributions and a headline. It never
consults safety findings. A run with a holder collision and five comfortable
load rows reads "no sample past the limit" on every row and says nothing
else. The repository already pinned this hazard: commit `b037568d`,
`freshness_does_not_outrank_a_collision`.

The shipped page is also summary-first by rule. `draw`
(`crates/rs_cam_viz/src/ui/sim_diagnostics.rs:22`) carries the DC6 comment —
one verdict line at the top, every dense section closed beneath it — and the
sentry is `crates/rs_cam_viz/tests/the_simulation_page_is_summary_first_dc6.rs`.
Five always-open histograms contradict it.

**Smallest correction.** The post-stage headline is the triage verdict. The
five rows sit below it. The load surface never speaks first.

---

## 2. Worth fixing before building

### W1 — Uniform treatment is right for the shape and wrong at the caption

The strongest case against uniform treatment is not the unset rows. Rule 2
handles those. It is the middle case.

The depth row draws a bar, prints a percentage and looks exactly like the
power row. The power limit comes from a machine power curve with `Ks`
corroborated twice. The depth limit is `0.20 × D` with no published source
(`WHERE_THIS_LANDS.md:135-141`). It is also the only limit that ever binds —
100 % of 54 recipes (`WHERE_THIS_LANDS.md:28-30`). The design draws the one
number nobody can defend identically to the one number everybody can.

The objection to badges is sound: a badge is a second axis competing with the
number for attention, and the five-way vocabulary at
`WHERE_THIS_LANDS.md:135-141` is a vocabulary to learn. The objection to
stating provenance at all is not sound, and the design conflates the two.

**Smallest correction.** Keep the row geometry identical. Put the provenance
in the axis caption, which already exists. `renderPrototype` prints `l.from`
under every bar (`derate_bench.html:1122`) — "Machine profile", "Not set by
any setting". Let that caption carry the source as well as the setting:
"Machine profile — measured power curve" against "Machine profile — rule of
thumb, no source". No badge, no colour, no ranking, same shape, one string
per row.

### W2 — The depth row's limit is wrong for adaptive operations and for finishing passes

`limitRows` hard-codes `DOC_ROUGHING = 0.20` (`derate_bench.html:943`) and
its hover text says the limit is 0.20 times the tool diameter
(`derate_bench.html:989-995`). The engine does not work that way.
`clamp_dpp_to_rigidity` (`crates/rs_cam_core/src/feeds/suggest.rs:2370`)
branches on the feeds family: an adaptive operation uses
`RigidityProfile::adaptive_doc_factor`, which is 1.5 to 2.0, not 0.20
(`RigidityProfile`, `crates/rs_cam_core/src/machine/mod.rs:55`, and the
wood-router preset at `crates/rs_cam_core/src/machine/mod.rs:172-179`). The
clamp also fires only on `PassRole::Roughing`.

So on an adaptive operation the design draws a red bar at seven times the
limit while the cut sits inside its own cap, and on a finishing pass it draws
a limit that does not apply. `impl Default for RigidityProfile`
(`crates/rs_cam_core/src/machine/mod.rs:64-76`) uses 0.25, so the prototype's
constant is one preset's value presented as the rule.

**Smallest correction.** Never recompute the cap in the surface. The engine
already emits the event:
`SuggestWarning::RoughingDepthClampedToRigidity { requested, capped }`
(`crates/rs_cam_core/src/feeds/suggest.rs:2392-2395`). Read the warning.

### W3 — The headline scalar is forbidden by the module it would read

The module header of `verdict`
(`crates/rs_cam_core/src/tool_load/verdict.rs:1-8`) opens with: there is no
scalar "load %" — a project-wide report is a vector of per-criterion
verdicts. The headline "this cut runs at 78 % of its closest limit"
(`renderPrototype`, `derate_bench.html:1088`) is a maximum over criteria,
which is defensible only while it names the criterion. It does name it in the
sub-line. Keep that binding. Never print the percentage without the
criterion, and never average the five.

### W4 — The population is not stated, so air moves and entry spikes enter the number

Three filters the engine applies and the design does not:

- `SimulationCutSample::is_cutting`
  (`crates/rs_cam_core/src/stock/simulation_cut.rs:170`) and
  `SimulationCutSample::in_transit_span`
  (`crates/rs_cam_core/src/stock/simulation_cut.rs:217`). The second field's
  own comment says extreme-value metrics skip transit samples to avoid
  lift-bridge artifacts. An unfiltered chip-thickness histogram reports
  roughly half the run under the rubbing floor on any job with normal linking
  moves.
- The steady-state feed filter.
  `UnmodeledReason::SteadyStateSamplesNotPresent`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:168`) exists because the band
  comparison is calibrated for steady-state cutting only.
- Entry transients. `EntrySpike`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:896`) carries a helix or
  plunge spike as an advisory and keeps it out of the trip decision.

`GatePopulation` (`crates/rs_cam_core/src/tool_load/verdict.rs:648`) is the
shipped answer. It states how many units reached the comparison and how many
the filters removed. `GatePopulation::is_vacuous`
(`crates/rs_cam_core/src/tool_load/verdict.rs:690`) is true when the verdict
rests on nothing, and `CriterionStatus::vacuity_clause`
(`crates/rs_cam_core/src/tool_load/verdict.rs:622`) is the one shared
operator-facing sentence.

This also settles the three-sample toolpath. The design's answer is that a
histogram of three samples obviously looks thin
(`renderPrototype`, `derate_bench.html:1157`). That relies on the operator
noticing. `GatePopulation` says it.

**Smallest correction.** Every row prints its population. Reuse
`vacuity_clause`; do not word a second one.

### W5 — A stale trace reads as a measurement

The limit comes from the current settings. The samples come from the captured
run. Change the machine after the run and the limit line moves under a frozen
distribution. The share past the limit then changes with no new measurement.

The core forbids this: a parameter, tool, model, stock or setup edit
invalidates the affected cached result chain, and an old result must not
survive under new inputs (`crates/rs_cam_core/CLAUDE.md`, core contracts).
`UnmodeledReason::StaleSimulation`
(`crates/rs_cam_core/src/tool_load/verdict.rs:155`) is the typed state.
`crates/rs_cam_viz/CLAUDE.md` adds that capture staleness derives from the
accepted run's capture revision.

**Smallest correction.** The post stage is not a toggle the operator picks.
It is an evidence state: no trace, stale trace, or fresh trace. A stale trace
draws no distribution.

### W6 — Edge cases that crash or print nonsense

- **No limit set on any row.** In `renderPrototype`, `scored` is empty,
  `scored[0]` is `undefined`, and `used(undefined)` throws
  (`derate_bench.html:1074-1079`). The headline and `worst.name` then fail.
  The state is reachable today: two of the five rows are already unset, and a
  drill cycle makes every milling gate
  `UnmodeledReason::NotApplicableForOp`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:148`, the variant at `:187`).
- **A floor with a value of zero.** `used` is `limit / value`
  (`derate_bench.html:1077`), so the row prints `Infinity %`. Any air sample
  or a zeroed `fz` produces it.
- **A limit of zero.** The filter at `derate_bench.html:1074` drops it from
  scoring, but `limitRows` still renders the row as "limit not set". A zero
  limit is not an unset limit. `None` means not measured and `Some(0.0)`
  means measured clean (`crates/rs_cam_core/CLAUDE.md`). The design collapses
  them.
- **A floor row reverses direction between stages.** Before the run the bar
  grows to the right as the chip approaches the floor. After the run
  `drawHistogram` paints the danger region left of the floor marker
  (`derate_bench.html:1057`). The same row points the operator two ways.
  `verdict_badge` already handles this case the other way: its comment at
  `crates/rs_cam_viz/src/ui/sim_diagnostics.rs:1158-1160` says that for
  chipload burn risk the peak is below the floor, so "% of cap" is
  misleading, and it skips the percent branch. The design prints the
  percentage the shipped code refuses to print.

---

## 3. Nitpicks

- `renderPrototype` prints the axis caption `0 … limit` in both stages
  (`derate_bench.html:1122`), but the post-stage right edge is at least 1.3
  times the limit. The caption mislabels the axis it sits under.
- The "Tool side force" row in `limitRows` reports force in newtons with no
  limit (`derate_bench.html:975-985`), and its hover says this page sets no
  limit for it. The engine does: `deflection::EXCEEDS_BOUND_MM`
  (`crates/rs_cam_core/src/tool_load/deflection.rs:64`) is 0.200 mm of tip
  displacement. The row shows the wrong quantity and inflates the "two of
  five are not set" claim to include a limit that exists.
- `rowAxisMax` depends on the predicted value
  (`derate_bench.html:1067-1070`), so the axis of a finished run moves when
  the operator edits a setting.
- `syntheticSamples` clamps at zero (`derate_bench.html:1024`), which
  manufactures samples exactly at the floor on floor rows. Synthetic data
  only.

---

## 4. What is sound and should not be touched

- **The two-stage split.** The operator does want a prediction. The
  repository already splits this way: Suggest and `FeedExplanation` before,
  the gates after. Keep the split. Remove only the claim that the two stages
  share an axis.
- **Rule 2 — an unset limit draws no bar and never appears comfortable.**
  Correct, and already shipped in `verdict_badge`. Add the reason:
  `UnmodeledReason` (`crates/rs_cam_core/src/tool_load/verdict.rs:148`)
  distinguishes "no simulation", "stale", "no vendor row", "does not apply to
  this operation" and "every sample was air or rapid".
- **Rule 3 — the hover names the setting that moves the limit.** A limit the
  operator cannot trace to a setting is a limit they cannot act on. Keep it
  exactly as written. It is the one part of the design that adds something
  `verdict_tooltip` does not already say.
- **A distribution instead of a peak.** The instinct is right. `EntrySpike`
  exists because one sample decided a verdict the rest of the run
  contradicted.

---

## 5. The simpler design

Most of this surface is computed, typed, on the MCP wire and already drawn.

`ToolpathLoadVerdict::criteria`
(`crates/rs_cam_core/src/tool_load/verdict.rs:316`) returns one
`CriterionStatus` per limit. Each carries `kind`, `state`, `confidence`,
`unmodeled_reason`, `sample_range`, `population`, `display_peak`, `unit` and
`exceeded` (`CriterionStatus`,
`crates/rs_cam_core/src/tool_load/verdict.rs:583`). Its doc states that this
list is the single inclusion point for the gating tier, so a gate added there
cannot be forgotten by export.

The simpler design is three edits to `verdict_badge` and `verdict_tooltip`,
not a new screen:

1. Put the provenance in the row caption (W1).
2. Print the population on the row instead of only in the tooltip (W4).
3. Name the order statistic with `ObservedStatistic::label` (S2).

For "how much of the run", `ModulationSummary::binding_constraint_distribution`
(`crates/rs_cam_core/src/tool_load/verdict.rs:127`) already reports the
fraction of touched moves each constraint bound, across seven typed
`BindingConstraint` variants
(`crates/rs_cam_core/src/tool_load/verdict.rs:35`). The GUI already reads it
in `crates/rs_cam_viz/src/ui/properties/mod.rs`. That is the share-of-run
question, already computed, already agreeing with the gates.

Two things are genuinely missing and neither is a histogram:

1. **The depth cap is not a gate.** It is a Suggest clamp that emits
   `SuggestWarning::RoughingDepthClampedToRigidity`
   (`crates/rs_cam_core/src/feeds/suggest.rs:2392-2395`). Promote the warning
   to a row, and take the factor from the branch in `clamp_dpp_to_rigidity`,
   not from a constant.
2. **Gantry push has no model.** It is register item T-10. It is an
   `Unmodeled` row with a typed reason and nothing else, until somebody
   measures the machine (`WHERE_THIS_LANDS.md:183-194`).

---

## 6. Building into a moving tree

Three questions were asked. Two have short answers and one does not.

### 6.1 Does the design assume a structure that is moving? Partly, and it does not matter

`FEEDS_WAVE.md:44` states it directly: "The two folders do not move. Only
their `use` lines change." `crates/rs_cam_core/src/feeds/` and
`crates/rs_cam_core/src/tool_load/` hold the design's main reading surface —
`verdict.rs`, `chipload.rs`, `power.rs`, `deflection.rs`, `display.rs`,
`suggest.rs`, `feed_explanation.rs` — and they stay where they are.

Two files the design reads did move today.
`crates/rs_cam_core/src/simulation_cut.rs` became
`crates/rs_cam_core/src/stock/simulation_cut.rs`, recorded at
`FEEDS_WAVE.md:38-40`, and `machine_kinematics.rs` became
`crates/rs_cam_core/src/machine/kinematics.rs`
(`planning/structure_2026-09-17/P2_REVIEW.md`, check 2). Both are
`SimulationCutSample` and `RigidityProfile` reads, which the design does not
own.

P2_REVIEW check 4a, 4b, 4c and 4d each report zero stale `crate::<old>` or
`rs_cam_core::<old>` paths anywhere in the tree. The compiler enforced the
rename. **No problem.**

One scheduling note, not a design fault. P2_REVIEW's "What this review did
not verify" item 3 states that the working tree holds uncommitted `feeds/**`
and `tool_load/**` edits from two other agents. `FEEDS_WAVE.md` §1 counts
callers in both folders and proposes deletions. Start after that wave lands.

### 6.2 Is now a bad time for a cross-crate type? No — and the type already exists

`CriterionStatus` already crosses the boundary: core defines it and
`crates/rs_cam_viz/src/ui/sim_diagnostics.rs` and
`crates/rs_cam_viz/src/ui/sim_op_list.rs` render it. The design needs no new
cross-crate type, because §5 reduces it to three edits inside that type's
existing rendering.

The restructure makes this easier rather than harder, and it supplies the
precedent. The module header of `tool_load::display`
(`crates/rs_cam_core/src/tool_load/display.rs:1-24`) explains why an
operator-facing display quantity belongs in core: the viewport heat-map's
measure was once a private helper in `rs_cam_viz::app::gpu_upload`, paired
with a private colour function in `rs_cam_viz::render::toolpath_render`, and
the pairing was the F-HEATMAP defect. `achieved_advance_per_tooth`
(`crates/rs_cam_core/src/tool_load/display.rs:40`) moved the measure into
core so that "the colour and the verdict agree" became a property a test can
assert. Colour stayed in viz.

That is the rule for any new quantity this design adds: the number goes in
`tool_load/`, the drawing stays in `rs_cam_viz/src/ui/`. P2_REVIEW check 7a
and 7b confirm the restructure is enforcing exactly these boundaries — no new
`_mut` function, and no dependency change in any `Cargo.toml`.

### 6.3 Can this codebase keep written provenance current? No. It has measured evidence that it cannot

This is the one place the churn changes the design.

**The measurement.** Doc comments under `crates/` cite 231 distinct
`planning/…` paths. 117 of them — 51 % — resolve to nothing in the tree
today. The citation count is 741.

The design's hover strings are provenance prose of the same class. Worse,
they hard-code values. `limitRows` writes "The floor is 0.025 mm per tooth"
and "It is 0.20 times the tool diameter" into English
(`derate_bench.html:957-996`). Those are duplicated constants. Nothing fails
when `RUBBING_FLOOR_MM_TOOTH` or `RigidityProfile::doc_roughing_factor`
changes.

This exact failure has already happened twice in the module the design would
read, and both are recorded in the code:

- `BindingConstraint::ChiploadMin`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:35`, the variant's doc at
  `:50-64`) carries a correction dated 2026-08-13: "This doc used to call
  itself the rubbing floor. It is not." The two quantities were 2.000× apart
  and pointed at opposite ends of the same band. The doc closes: "A docstring
  that names a constant the code does not use is a lie a reader then cites."
- `ChipBoundsSource::row_id`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:861-874`) carries the H4
  finding: the chipload diagnostic hard-coded `"vendor_lut"` regardless of
  source, so a live session saw `row_id: vendor_lut` and `extrapolated: true`
  in one breath. "A citation that names a row the bounds did not come from is
  worse than no citation."

A third is open right now. `FEEDS_WAVE.md:70`, row FW-09, records a banner
comment at `crates/rs_cam_core/src/tool_load/verdict.rs:525-527` claiming the
typed verdict scaffolding has "No consumers". `LoadState` has 35 references
outside its file, `ChiploadVerdict` has 181, and the GUI reads all three.

P2_REVIEW's own fix list is the same shape from the other end. Findings F1
through F4 are **all** prose: `CREDITS.md` keeps 13 old paths, `lib.rs:41`
keeps an orphan comment, four test doc comments keep dead module names, and
four live documents name five files that exist nowhere. Zero findings are
code. The restructure moved 106 files and the compiler caught every code
citation and not one prose citation.

**So the answer is no, and the design must not rely on prose.** The
correction is small and it is already the house pattern:

- A number in a hover comes from the constant, formatted at render time.
  Never retype it. `verdict_tooltip` already does this — it calls
  `vacuity_clause` rather than wording the sentence itself, with the stated
  reason that the GUI, CLI, MCP and diagnostics list cannot then drift.
- A provenance phrase is a typed value, not a string literal in the UI.
  `Confidence::Approximate(String)` already carries "which input is
  approximate" from the producer that knows.
- If a hover must cite a document, cite the register item (T-10), not a path.
  A register item survives a purge; `planning/…` paths demonstrably do not.

One qualification, in the design's favour. Its rule 3 names a **setting**,
not a document — "to move this limit, change the machine or the tool
diameter". A setting is a live symbol a reader can find. That half of rule 3
is durable and is the best idea in the design. It is the "where it came from"
half, and the hard-coded numbers, that rot.

---

## Summary of corrections, ranked

| # | Finding | Correction |
|---|---|---|
| S1 | The surface exists; rule 1 removes shipped confidence rendering | Get the removal ruled explicitly; prefer W1's caption form |
| S2 | The shared axis is not shared, and holds two quantities | Drop rule 4; name each stage's quantity and statistic; fix the power producer first |
| S3 | The share is a sample count, wrong for two metrics | Per-row statistic; time-weight where duration matters; peak where it does not |
| S4 | The page-one claim skips triage | Triage headline above the five rows |
| W1 | Provenance is invisible on the middle case | Put the source in the axis caption |
| W2 | The depth limit is wrong for adaptive and finishing | Read `SuggestWarning::RoughingDepthClampedToRigidity` |
| W3 | A scalar load % is forbidden | Never print the percent without the criterion |
| W4 | No population stated | Print `GatePopulation`; reuse `vacuity_clause` |
| W5 | A stale trace reads as a measurement | Make the post stage an evidence state, not a toggle |
| W6 | Four edge cases crash or invert | Guard the empty set, the zero divisor, the zero limit, the floor direction |
| 6.3 | Prose provenance rots — 117 of 231 cited paths are dead | Format hovers from constants and typed values, never literals |
