# The limits surface — UI plan and implementation plan

Rests on three independent surveys, all verified against the code:
`SURVEY_UI.md`, `SURVEY_CORE.md`, `REVIEW_DESIGN.md`.

**The headline: most of this surface already exists and works.** The plan is
now four small additions and two operator rulings, not a build.

---

## 1. What the surveys changed

`verdict_badge` (`crates/rs_cam_viz/src/ui/sim_diagnostics.rs`) already draws
one row per limit from `CriterionStatus`, and it already implements three of
the four rules the prototype proposed:

| Prototype rule | Status |
|---|---|
| A limit that is not set never looks comfortable | **Shipped.** Vacuous paints `∅` dim; `Unmodeled` paints `—` |
| Where the limit came from sits behind a hover | **Shipped.** `verdict_tooltip`, wording from core's `vacuity_clause` |
| Every limit on one comparable scale | **Shipped.** `pct_of_cap` |
| Every limit drawn identically, no confidence mark | **NOT shipped — and it is a removal** |

The prototype was 80 % a reimplementation. Worse, on the one point it was
built to fix it is behind the shipped code: `verdict_badge` deliberately
refuses to print a percent for the chipload floor, because the peak sits
*below* the floor and "% of cap" is misleading there. The prototype prints
exactly that percent.

**So the design question is no longer what to build. It is what to add, and
what to remove.**

---

## 2. Two rulings for the operator

Neither is the engine's to make.

### Ruling A — uniform rows means deleting a confidence mark

The request was that every limit be treated the same, with provenance in a
hover. The codebase renders `Confidence::Approximate` as a `≈` suffix and a
milder colour, and `Confidence`'s own doc instructs the UI to render it
differently.

So "treat them all the same" is a **removal of shipped behaviour**, not an
addition. It should be ruled on as one.

**Recommendation: do it, but move the mark rather than delete it.** The `≈`
is answering a real question — is this number trustworthy — and the answer
belongs in the hover with the rest of the provenance, where it can say *which*
input is approximate. `Confidence::Approximate(String)` already carries that
string and the badge throws it away.

### Ruling B — the ban on a predicted power bar rests on expired evidence

`no_power_gauge_returns_to_the_card_g_chipverdict` bans the bar because the
readout peaked at 23.6 % across the shipped matrix. That was measured at
`921aa0e3`, **before** R1 (`73b84d69`) rebuilt the power model about 8.6x
higher.

Re-measured after R1, 162 recipes: median 17.8 %, p90 89.4 %, peak 100.0 %,
with 25 % of recipes above half scale.

**Recommendation: reverse it, and replace the assertion rather than remove
it.** The sentry should keep watching that the readout is informative — that
utilisation still spreads across the range — so a future model change that
flattens it again is caught. A ban whose reason expired becomes a test of the
reason.

**This session must not make that change itself.** It wrote the sentry, and a
sentry deleted by its author on the author's own re-measurement is re-pinning
a test to fit the work.

---

## 3. The UI plan

### Where it lives

**Extend `draw_tool_load_badges`, and give Readiness the same rows.**

Readiness owns the question by name — `Workspace::description` reads "Is this
safe to cut?" — and is the only workspace whose geometry fits a row stack: one
560-point centred column, no side rails. But its "Tool load" row counts
TOOLPATHS, not limits, so it answers a different question than its title
promises.

`draw_tool_load_badges` is the only per-limit implementation that exists. It
is the extension target. The Readiness "Tool load" row and its three
`CountPill::verdict` calls are the replacement candidate — which satisfies the
standing rule that a new panel must replace something.

**The inspector rail is not a candidate.** `PANEL_WIDTH` pins it to 240 points
on every tab, and a stack of distributions does not fit.

### The two stages

They live in two workspaces today: predicted on the Feeds tab, measured on
Simulation and Readiness. **Do not merge them into one surface.** Each answers
a different question at a different moment, and the surveys show the two
stages do not even hold the same quantity — the gate observes
`commanded_fpt × predicted_feed / commanded_feed`, not the commanded value.

So: same row shape in both places, no shared axis, and no claim that one
predicts the other.

### The shape of a row

Keep what `verdict_badge` does. Add the distribution only where a distribution
exists, and only after a simulation. Before a simulation the row is what it is
today.

---

## 4. What is genuinely missing

Four things. Two are small, one is medium, one is blocked.

### 4a. Depth of cut has no criterion — SMALL

There is no `CriterionKind` for it, and no ceiling in the GUI to draw against.
The cap exists as `SuggestWarning::RoughingDepthClampedToRigidity`, which is a
warning, not a limit.

This is the limit that sets the depth on **100 % of recipes**, and it is the
one the operator cannot see as a limit at all.

**Do:** promote the rigidity cap to a criterion so it renders beside the
others. It will read as the binding row on nearly every cut, which is the
point.

### 4b. Gantry push — BLOCKED, and it should still appear

`lateral_cutting_force` (`feeds/force.rs`) already returns the newtons. Only
the machine-side capacity is missing, and no manufacturer publishes one.

**Do:** add the row now as `Unmodeled(UnmodeledReason::NotImplemented)`. That
arm already exists and already paints `—`. A limit that is absent should be
visibly absent, not silently missing from the list. It costs one enum variant
and no physics.

### 4c. Per-sample spindle power — MEDIUM

Power and deflection are **not on the sample**. Both gates recompute per
sample at gate time. Every input is stored, so a distribution is buildable,
but it needs a recompute pass rather than a read.

**Do this only after 4a and 4b**, and only if the distributions earn it.

### 4d. The distribution itself — MEDIUM, with two constraints from the survey

- A **fixed-size array of bins, never a `BTreeMap`.** Maps cost 50 % in the
  aggregation bench.
- **Weight by `segment_time_s`,** which is on every sample and which the
  prototype ignored entirely.
- **State the population.** Carry `GatePopulation` so a histogram over zero
  samples is distinguishable from a clean run. This is the X-VAC rule, and it
  is the reason the existing badge is trustworthy.
- Pick the statistic **per metric, not uniformly.** `ChiploadStatistic`
  already does this: `MedianLow` against the floor, `PeakHigh` against the
  ceiling. One excursion is total loss for deflection; a median is right for
  burn. The prototype's single "share of the run past the limit" is wrong for
  at least two of the five rows.

Traces run 100k–600k samples. A JSON dump has reached 1.2 GB and filled a
disk, so the histogram must be a summary, computed once and carried — never a
second copy of the trace.

---

## 5. Order of work

```
1. Ruling A and Ruling B          <- operator, not the engine
2. Gantry push as an Unmodeled row     small, no physics
3. Depth of cut as a criterion         small, and it is the row that binds
4. Re-look at the surface with 2 and 3 in it
5. Distributions, per-metric statistic, time-weighted, population stated
6. Per-sample power, only if 5 earns it
```

Steps 2 and 3 are worth doing whatever is decided about 5 and 6. They make the
two limits the operator most needs visible, and neither needs new physics.

**Do not start at step 5.** The distribution is the interesting part and the
least valuable. The depth cap sets every recipe and cannot currently be seen.

---

## 6. What the prototype got right, and what to take from it

Right, and worth keeping:

- Distribution over peak, as an instinct. A peak cannot say whether a breach
  is one corner or half the job.
- The hover naming the **setting** rather than a document. A setting is a live
  symbol; a document path is not. 51 % of the `planning/…` paths cited in doc
  comments under `crates/` now resolve to nothing, so this distinction is the
  best idea in the design.
- The two-stage split.

Wrong, and corrected above:

- It prints a percent for the chipload floor, which the shipped badge refuses.
- It counts samples, not time.
- It hard-codes `0.025` and `0.20 × D` into English prose. Hover text must be
  formatted from the constants and the typed values, never from literals —
  `verdict_tooltip` already does this by calling `vacuity_clause`.
- It proposed a shared axis across the two stages, which would draw a known
  3.5x over-read as "the run contradicted the prediction".

The prototype stays in `derate_bench.html` as a design record. **It is not the
implementation target.** `verdict_badge` is.

---

## 7. Two refinements from the final core survey

Neither changes the order of work.

**Chip thickness exists in four distinct forms on the sample**, and they are
not interchangeable: `SimulationCutSample::effective_chip_thickness_mm` is
what the gate reads, `Engagement::mean_chip_thickness_mm` and
`::peak_chip_thickness_mm` are arc statistics, and
`SimulationCutSample::chipload_mm_per_tooth` is the commanded advance. A
distribution must name which one it drew. This is the same confusion
`feed_explanation` was written to end — four disagreeing chipload numbers with
nothing saying they were different quantities.

**No scalar summary carries a chip-thickness extreme** except
`KinematicsSummary`, and `peak_engagement` is computed by the accumulator but
never published on the per-toolpath summary. So a distribution at toolpath
scope cannot be assembled from existing summary fields. It must come from the
gate's own `ChiploadMetric`, which already carries the observed value, the
named statistic and the bounds together. That is the right source anyway.

## 8. A note on citations in this package

The three surveys were run while another session was restructuring the tree.
They were instructed mid-flight to cite by SYMBOL first and path second.

That was not precautionary. Two citations moved during a ten-minute survey —
a comment in `feeds/mod.rs` and `PlungeStressWarning` — and a third could not
be separated from the surveyor's own mis-citation, which the survey says
plainly rather than quietly correcting.

**Every line number in this package should be treated as approximate. Every
symbol name should be treated as exact.** Anyone acting on these documents
should search for the symbol, not open the line.

---

# 9. The proposed approach (2026-09-17)

Written after the surveys, and it replaces the confidence-tag idea entirely.

## The concept

**A limit has two independent properties: how bad the consequence is, and how
much the bound is trusted. They are not the same axis, and the codebase
already knows it.**

`ChipBoundsSource::low_side_is_advisory` is the proof. When the chipload
floor's provenance is weak, a cut below it becomes a structured **advisory
inside `Within`** rather than flipping to `Exceeds`. It is reported, it is
visible, and it does not refuse a g-code export. The comment on the HIGH side
states the other half:

> The HIGH (breakage) side stays hard for every source — over-thick chips
> break teeth regardless of how the bound was derived.

So: breakage is hard whatever the source, because the consequence is physical.
Burn is advisory when the source is weak, because the consequence is gradual
AND the bound is uncertain. Two axes, already separated, already shipped.

**This is why the confidence badge was the wrong idea, and why the operator
was right to reject it.** A badge asks the reader to hold the confidence in
their head and discount the number themselves. The mechanism above does it for
them: a bound the engine does not trust **cannot stop the job**. The behaviour
carries the meaning, so the label does not have to.

## What follows

### Depth of cut joins as a criterion with an ADVISORY bound

The rigidity cap decides the depth on 100 % of recipes and the operator cannot
see it as a limit. Promote it.

But `enforce_load_policy` refuses export on ANY exceeded criterion, with no
per-criterion scoping. `0.20 × diameter` on a Ø6 tool is 1.2 mm, and plenty of
people deliberately cut deeper. **Gating on it would block working jobs on a
number with no source.**

So it joins the way the chipload floor already does: visible, comparable, and
advisory. It will read as the binding row on nearly every cut, which is the
point — the operator sees that the thing deciding their depth sits at 350 %
while the measured power limit sits at 40 %.

**And the day somebody measures the machine, the same row becomes hard with no
UI change.** The advisory flag is a property of the bound's provenance, not of
the display.

### Gantry push joins as `Unmodeled(NotImplemented)`

The force is already computed by `feeds::force::lateral_cutting_force`. Only
the machine-side capacity is missing. The arm already exists and already paints
`—`. An absent limit should be visibly absent.

### Ruling A dissolves

The question was whether to delete the `≈` confidence mark. The answer is that
confidence should change what the system DOES, not how a row is decorated.
Once a weak bound cannot gate, the row only has to say what happened. The `≈`
can go, and nothing is lost, because the information moved into behaviour.

The `Approximate(String)` payload still belongs in the hover — it says WHICH
input is approximate, and the badge throws it away today.

### Distributions move to "probably not yet"

Put the depth row next to the power row on one scale and the comparison does
the work a histogram was being asked to do. Revisit only if that surface
leaves a real question unanswered.

## Order

```
1. Ruling B — the power-bar ban            operator
2. Gantry push as Unmodeled                small
3. Depth of cut as an advisory criterion   small
4. Stop reporting power at a depth that will not be cut   correctness
5. LOOK. Then decide whether anything else is needed.
```

Step 4 is the one correctness defect found in the whole survey: `power_kw` is
computed before `clamp_dpp_to_rigidity`, so a Ø12 cut reports a figure for a
depth 3.5x deeper than it will cut. It is the same shape as the stale-geometry
defect that got the FEED pinned in 2026-08; power was never pinned with it.

Nothing here needs new physics, a new chart, or a new panel.
