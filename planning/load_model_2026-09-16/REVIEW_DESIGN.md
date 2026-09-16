# Adversarial review — the five-limit surface

Reviewed 2026-09-17. Read only. The subject is
`planning/load_model_2026-09-16/WHERE_THIS_LANDS.md` §4 and the prototype in
`planning/load_model_2026-09-16/derate_bench.html:933-1180`.

The design commits to four rules:

1. Every limit draws identically. No badge marks one as better evidenced.
2. A limit that is not set draws no bar and never appears comfortable.
3. The source of a limit lives behind a hover and names the setting that
   moves it.
4. The scale is shared between the two stages, so the operator can compare
   the prediction to the run.

Rule 4 is wrong. Rules 1 and 2 need one correction each. Rule 3 is sound.

---

## 1. Findings that would sink it

### S1 — The shared scale is not shared, and the two stages do not hold the same quantity

Two separate faults sit on top of each other.

**The prototype does not do what it claims.** The pre-stage bar fills the row
element to `f × 100 %`, where the full element means the limit
(`derate_bench.html:1113`). The post-stage histogram spans `rowAxisMax`,
which is `max(limit, value × 1.45) × 1.3` — never less than 1.3 times the
limit (`derate_bench.html:1067-1070`). The limit marker therefore sits at
77 % of the width or less (`derate_bench.html:1109-1110`). The same row
means "limit at the right edge" before the run and "limit at three quarters"
after it. The comment at `derate_bench.html:1105-1106` states the opposite
of the code below it.

That fault is small. The fault under it is not.

**The prediction and the samples are different quantities on two of the five
rows.** The repository already established this and built a type to stop it.
`crates/rs_cam_core/src/feeds/feed_explanation.rs:12-30` records a live
session that produced four chipload numbers for one operation, no two
agreeing, with nothing on screen saying they were four different quantities.
The gate observes `commanded_fpt × predicted_feed / commanded_feed`
(`feed_explanation.rs:47-49`). Suggest's number is the commanded value. They
differ by the achieved-feed ratio, always, on a correct run.

The power row is worse. `WHERE_THIS_LANDS.md:38-47` records that the
recommendation computes `power_kw` at the calculator's depth, before
`clamp_dpp_to_rigidity` cuts that depth by about 3.5 times
(`crates/rs_cam_core/src/feeds/suggest.rs:2370-2396`). A shared axis draws
that 3.5× gap as a prediction that the run contradicted. The operator learns
nothing true. The producer is wrong, not the machine.

So the design's central promise delivers a picture of a known defect.

**What the design says when the two disagree: nothing.** It shows two
pictures and a sub-line that says the share past the limit is what matters.

**Smallest correction.** Drop rule 4. Do not share an axis between stages.
Each stage states its own quantity by name, and the post stage names the
order statistic it used. `crates/rs_cam_core/src/feeds/feed_explanation.rs`
already carries `ObservedStatistic::{Median, Peak}` with operator-facing
labels, and it exists because "chipload" alone does not say which is on
screen (`feed_explanation.rs:69-90`). Then fix the power producer —
`WHERE_THIS_LANDS.md:205` step 2 — before any comparison ships.

### S2 — "Share of the run past the limit" is a sample count, and it is the wrong statistic for two of the five metrics

The prototype computes the share as
`samples.filter(...).length / samples.length` (`derate_bench.html:1133`).
That is a count of samples, not a share of the run. Every sample carries
`segment_time_s`
(`crates/rs_cam_core/src/stock/simulation_cut.rs`, `SimulationCutSample`), so
the time weight is available and unused. A slow deep corner and a fast
shallow pass contribute equally. The foot text at `derate_bench.html:1153`
asks "is this one corner or half the job?" — which is a time question — and
answers it with a count.

The deeper fault is that duration does not matter for every limit.

- **Gantry push.** The drive loses position once. Every coordinate after
  that is wrong and the part is scrap. A 0.02 % share is a total loss.
- **Tool deflection.** Past the fracture load the tool breaks. There is no
  such thing as breaking for 0.3 % of the run.
- **Chip thickness (a floor).** Rubbing is thermal. Duration is exactly what
  matters.
- **Spindle power.** The motor's thermal mass absorbs a short excursion.
  Duration matters.

The core already made this distinction and made it per side.
`crates/rs_cam_core/src/tool_load/verdict.rs:806-818` defines
`ChiploadStatistic`: the burn side uses the **median** of in-cut samples, and
the breakage side uses the **per-sample peak**. The design would overwrite a
reasoned, shipped choice with one statistic for all five rows.

A brief catastrophic excursion is also invisible in the drawing. Any sample
above `axisMax` is clamped into the last bin (`derate_bench.html:1047`), so a
sample at five times the limit draws in the same column as one at 1.9 times.

**Smallest correction.** Each row declares its own headline statistic, and
the row prints the statistic's name. Time-integrated metrics report a
time-weighted share. Instantaneous-failure metrics report the peak and the
count of excursions, never a share. Keep the histogram as the picture; change
the number above it.

### S3 — The surface claims the page-one position and does not read triage

`crates/rs_cam_viz/CLAUDE.md` states that simulation presentation reads
bounded triage first, and that `NotMeasured` is an abstention, not a healthy
zero. `crates/rs_cam_core/CLAUDE.md` states that
`ProjectSession::simulation_triage` is read before raw issue counts.
`crates/rs_cam_core/src/stock/sim_triage.rs:215-230` calls `SimulationTriage`
"the page-one answer", with `measurability` read first, then class A safety,
then class B actions.

The post-stage screen shows five distributions and a headline. It never
consults safety findings. A run with a holder collision and five comfortable
load rows reads "no sample past the limit" in every row and says nothing
else. The repository already pinned this exact hazard: commit `8cac2857`,
`freshness_does_not_outrank_a_collision`.

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

The repository has already ruled on this axis twice, and in code, not prose:

- `crates/rs_cam_core/src/tool_load/verdict.rs:199-211` defines `Confidence`
  and states that the UI **must** render `Approximate` differently from
  `Validated`, so users do not anchor on it.
- `crates/rs_cam_core/src/tool_load/verdict.rs:871-884` lets provenance
  change behaviour, not only appearance:
  `ChipBoundsSource::low_side_is_advisory` downgrades a burn-side trip to an
  advisory because the bound is weakly provenanced.

Does the operator's objection survive? Partly. A badge is a second axis that
competes with the number for attention, and the five-way vocabulary in
`WHERE_THIS_LANDS.md:135-141` is a vocabulary to learn. The objection to
badges is sound. The objection to stating provenance is not, and the design
conflates them.

**Smallest correction.** Keep the row geometry identical. Put the provenance
in the axis caption, which already exists. `derate_bench.html:1122` prints
`l.from` under every bar — "Machine profile", "Not set by any setting". Let
that caption carry the source as well as the setting: "Machine profile —
measured power curve" against "Machine profile — rule of thumb, no source".
No badge, no colour, no ranking, same shape, one string per row.

### W2 — The depth row's limit is wrong for adaptive operations and for finishing passes

The prototype hard-codes `DOC_ROUGHING = 0.20` (`derate_bench.html:943`) and
the hover says the limit is 0.20 times the tool diameter
(`derate_bench.html:989-995`). The engine does not work that way.
`clamp_dpp_to_rigidity` branches on the feeds family
(`crates/rs_cam_core/src/feeds/suggest.rs:2380-2387`): an adaptive operation
uses `adaptive_doc_factor`, which is 1.5 to 2.0, not 0.20
(`crates/rs_cam_core/src/machine/mod.rs:56-75`, `:172-179`). The clamp also
applies only to `PassRole::Roughing`.

So on an adaptive operation the design draws a red bar at seven times the
limit when the cut is inside its own cap, and on a finishing pass it draws a
limit that does not apply. The default profile uses 0.25, not 0.20, so the
constant is one preset's value presented as the rule.

**Smallest correction.** Never recompute the cap in the surface. The engine
already emits the event:
`SuggestWarning::RoughingDepthClampedToRigidity { requested, capped }`
(`crates/rs_cam_core/src/feeds/suggest.rs:2392-2395`). Read the warning.

### W3 — The headline scalar is forbidden by the module it would read

`crates/rs_cam_core/src/tool_load/verdict.rs:1-8` opens with: there is no
scalar "load %" — a project-wide report is a vector of per-criterion
verdicts. The headline "this cut runs at 78 % of its closest limit"
(`derate_bench.html:1088`) is a maximum over criteria, which is defensible
only while it names the criterion. It does name it in the sub-line. Keep that
binding. Never print the percentage without the criterion, and never average
the five.

### W4 — The population is not stated, so air moves and entry spikes enter the number

Three filters the engine applies and the design does not:

- `is_cutting` and `in_transit_span` on each sample
  (`crates/rs_cam_core/src/stock/simulation_cut.rs`). The field's own comment
  says extreme-value metrics skip transit samples to avoid lift-bridge
  artifacts. An unfiltered chip-thickness histogram reports roughly half the
  run under the rubbing floor on any job with normal linking moves.
- The steady-state feed filter. `UnmodeledReason::SteadyStateSamplesNotPresent`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:162-168`) exists because the
  band comparison is calibrated for steady-state cutting only.
- Entry transients. `EntrySpike`
  (`crates/rs_cam_core/src/tool_load/verdict.rs:884-910`) carries a helix or
  plunge spike as an advisory and keeps it out of the trip decision.

`GatePopulation { contributing, offered, unit }`
(`crates/rs_cam_core/src/tool_load/verdict.rs:648-660`) is the shipped answer.
It states how many samples reached the comparison and how many the filters
removed. `is_vacuous()` is true when the verdict rests on nothing, and
`vacuity_clause()` is the one shared operator-facing sentence.

This also settles the three-sample toolpath. The design's answer is that a
histogram of three samples obviously looks thin (`derate_bench.html:1157`).
That relies on the operator noticing. `GatePopulation` says it.

**Smallest correction.** Every row prints its population. Reuse
`vacuity_clause()`; do not word a second one.

### W5 — A stale trace reads as a measurement

The limit comes from the current settings. The samples come from the captured
run. Change the machine after the run and the limit line moves under a frozen
distribution. The share past the limit then changes with no new measurement.

The core forbids this: a parameter, tool, model, stock or setup edit
invalidates the affected cached result chain, and an old result must not
survive under new inputs (`crates/rs_cam_core/CLAUDE.md`, core contracts).
`UnmodeledReason::StaleSimulation`
(`crates/rs_cam_core/src/tool_load/verdict.rs:152-155`) is the typed state.
`crates/rs_cam_viz/CLAUDE.md` adds that capture staleness derives from the
accepted run's capture revision.

**Smallest correction.** The post stage is not a toggle the operator picks.
It is an evidence state: no trace, stale trace, or fresh trace. A stale trace
draws no distribution.

### W6 — Edge cases that crash or print nonsense

- **No limit set on any row.** `scored` is empty, `scored[0]` is `undefined`,
  and `used(undefined)` throws (`derate_bench.html:1074-1079`). The headline
  and `worst.name` then fail. This state is reachable today: two of the five
  rows are already unset, and a drill cycle makes every milling gate
  `NotApplicableForOp`.
- **A floor with a value of zero.** `used = limit / value` divides by zero
  (`derate_bench.html:1077`) and the row prints `Infinity %`. Any air sample
  or a zeroed `fz` produces it.
- **A limit of zero.** The filter at `derate_bench.html:1074` drops it from
  scoring, but `limitRows` still renders the row with `limit not set`. A zero
  limit is not an unset limit. `None` means not measured and `Some(0.0)`
  means measured clean (`crates/rs_cam_core/CLAUDE.md`). The design collapses
  them.
- **A floor row reverses direction between stages.** Before the run the bar
  grows to the right as the chip approaches the floor. After the run the
  danger zone is the region left of the floor marker
  (`derate_bench.html:1057`). The same row points the operator two ways.

---

## 3. Nitpicks

- The axis caption prints `0 … limit` in both stages
  (`derate_bench.html:1122`), but the post-stage right edge is at least 1.3
  times the limit. The caption mislabels the axis it sits under.
- The "Tool side force" row reports force in newtons with no limit
  (`derate_bench.html:975-985`), and the hover says this page sets no limit
  for it. The engine does: `EXCEEDS_BOUND_MM = 0.200`
  (`crates/rs_cam_core/src/tool_load/deflection.rs:64`), a tip displacement
  in millimetres. The row shows the wrong quantity and inflates the "two of
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
  the gates after. Keep the split. Remove only the claim that the two share
  an axis.
- **Rule 2 — an unset limit draws no bar and never appears comfortable.**
  This restates X-VAC and the abstention rule correctly, and it is the
  strongest part of the design. Add the reason: `UnmodeledReason` already
  distinguishes "no simulation", "stale", "no vendor row", "does not apply to
  this operation" and "every sample was air or rapid".
- **Rule 3 — the hover names the setting that moves the limit.** A limit the
  operator cannot trace to a setting is a limit they cannot act on. Keep it
  exactly as written.
- **A distribution instead of a peak.** The instinct is right. `EntrySpike`
  exists because one sample decided a verdict that the rest of the run
  contradicted.

---

## 5. The simpler design

Most of this surface is already computed, typed and on the MCP wire.

`ToolpathLoadVerdict::criteria()`
(`crates/rs_cam_core/src/tool_load/verdict.rs:312-345`) returns one
`CriterionStatus` per limit. Each one carries `kind`, `state`, `confidence`,
`unmodeled_reason`, `sample_range`, `population`, `display_peak`, `unit` and
`exceeded` (`verdict.rs:583-607`). The doc comment states that this list is
the single inclusion point for the gating tier, so a gate added here cannot
be forgotten.

The simpler design renders that list. One row per `CriterionStatus`, in
`criteria()` order:

- the state, the peak and the unit — already typed;
- the population clause — already worded, once, in `vacuity_clause()`;
- the unmodeled reason when there is one — already typed and localisable;
- the confidence — already typed, with an explicit instruction to render
  `Approximate` differently.

That answers "can my machine do this cut" with numbers the export gate acts
on, so the screen and the refusal cannot disagree.

For "how much of the run", `ModulationSummary.binding_constraint_distribution`
(`crates/rs_cam_core/src/tool_load/verdict.rs:239-250`) already reports the
fraction of touched moves each constraint bound, across seven typed
`BindingConstraint` variants (`verdict.rs:34-88`). That is the share-of-run
question, already computed, already agreeing with the gates.

Two things are genuinely missing and neither is a histogram:

1. **The depth cap is not a gate.** It is a Suggest clamp that emits
   `SuggestWarning::RoughingDepthClampedToRigidity`. Promote the warning to a
   row, and take the factor from the branch in `clamp_dpp_to_rigidity`, not
   from a constant.
2. **Gantry push has no model.** It is register item T-10. It is an
   `Unmodeled` row with a typed reason, and nothing else, until somebody
   measures the machine (`WHERE_THIS_LANDS.md:183-194`).

Build the table. Add the histogram later, per row, where the row's own
statistic says a distribution is the right picture.

---

# Verification by the commissioning session, 2026-09-17

The review above was checked before any of it was acted on. Four claims carry
the weight; all four hold. Symbols are given because the tree is being
restructured and line numbers will not survive.

| Claim | Verdict |
|---|---|
| `CriterionStatus` already carries the row model | **Holds** |
| Core already picks a statistic per metric | **Holds** |
| The two stages do not hold the same quantity | **Holds** |
| The prototype counts samples, not time | **Holds** |

**1. `CriterionStatus` (`tool_load/verdict.rs`) is almost exactly the row the
design proposed.** It carries `kind`, `state`, `confidence`,
`unmodeled_reason`, `population`, `display_peak`, `unit` and `exceeded`, plus
`vacuity_clause()` — one shared sentence so the wording cannot drift between
the GUI, the CLI, MCP and narration.

Its own doc comment describes the defect the design was built around:

> Measured 2026-08-05: three gates returned `Within` with `sample_range 0..0`
> — indistinguishable on every surface from a measured clean cut. The verdict
> was *not* wrong; it was **vacuous**, and nothing said so.

So the codebase solved absence-rendered-as-a-reading for load gates in August
2026, with a typed population field. The prototype reinvented a weaker form of
it — a hatched bar and the words "limit not set" — because the design started
from the physics and never read the verdict surface. That is the single
biggest correction here, and it makes the work smaller.

**2. `ChiploadStatistic` already picks the statistic per side.** `MedianLow`
against the LUT minimum, `PeakHigh` against the maximum. The engine already
holds the position the review argues for: a median for the slow-damage case, a
peak for the sudden-failure case. The prototype's one uniform "share past the
limit" for every metric contradicts a decision this repository already made.

**3. The two stages observe different quantities.** `feed_explanation.rs`
states the gate's observation as
`commanded_fpt × predicted_feed / commanded_feed`, which is not the commanded
value. That whole type exists because one session produced four disagreeing
chipload numbers with nothing saying they were different quantities. Drawing a
prediction and an observation on one shared axis, unlabelled, is that defect
again.

**4. The prototype counts samples, not time.** `segment_time_s` appears
nowhere in `derate_bench.html`. Ten slow samples and ten fast ones weigh the
same.

## One thing the review understated

Provenance is already typed as well. `ChipBoundsSource` distinguishes
`VendorLut`, `VendorLutExtrapolated` and a single-point row whose implied floor
is fabricated. `Confidence::Approximate(String)` carries which input is
approximate.

This answers the question put to the reviewer about whether written provenance
rots in this repository. It does — 293 doc comments now cite deleted planning
files. But the answer is not to write better sentences. **The provenance is
already data, so the hover text should be derived from these enums rather than
hand-written per row.** A derived string cannot go stale against the value it
describes.

## What this changes

The design is now mostly **rendering a type that already exists**, not building
a new one. Two things are genuinely missing and neither is a histogram:

- the depth cap, which today is only a `SuggestWarning`, and
- gantry push, which needs an `Unmodeled` row until somebody measures a machine.
