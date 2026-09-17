# Resume plan — the load-model programme

Written 2026-09-18. **Read this file first.** It is self-contained: a cold
session needs nothing else to restart, and it names the documents to open only
when a step needs them.

The programme began with one question: *how do we simplify the power readings
an operator sees?* Everything below still serves that. The detour into the
load model happened because the surface would otherwise have displayed numbers
whose inputs nobody had checked.

---

## 1. Where it stands

Shipped and verified:

| Commit | What |
|---|---|
| `a7c17be7` | The power ladder — RPM, depth, width, then feed as a LAST rung |
| `809d28b9` | T-11 — feed modulation reads mm, not a fraction of another quantity |
| `ad2f749b` | T-12 — the depth recommendation reaches all 24 operations |
| `70a3db27` | A failed collision check no longer reads as a clean one (CLI) |
| `6aca2ffd` | T-16 — the false bending-diameter citation removed |
| `9405f3f6` | Three surveys, and T-18 discovered |
| `93dd145c` | **T-17 — the deflection integrator bends a flute-relieved section** |
| `6d69c25a` | T-17 closed in the register |
| `0a510215` | The fluted-section derivation the code cites |
| `6a3ba42c` | The corridor fixture re-derived into the middle of its window |
| `59822cab` | **T-4 — a refused deflection prediction is a type, not a zero** |
| `d47a04d8` | **T-18 — every post-Step-9 feed lift caps on the COMMANDED cutting ceiling** |
| `6a9330dc` | **T-9 — a clamped feed ships at or below its ceiling; the export validator gets the travel rate** |

T-17 verification: core lib 2503/0, sentry 5/5, `literature_matrix` 21/21,
`literature_parity` 24/24, clippy clean, fmt clean.

**Update 2026-09-18:** the outstanding corridor check below was run. It was
red (`EdgeForceOverBudget` on `LONG_AND_THIN`), exactly as predicted, and
was repaired at `6a3ba42c` by re-deriving the fixture into the middle of its
window. `CORRIDOR_REPAIR.md` has the derivation. The paragraph below is kept
as the record of the prediction.

**One check was outstanding and it was real.** `the_corridor_bounds_the_band_g_corridor`
in `rs_cam_viz` was never run. It PANICS rather than drifts if the force budget
crosses the edge force, and T-17 made every tool 2.441x more compliant, so it
is a plausible red. It was skipped because a second session was rewriting
`rs_cam_viz`. **Run it first thing:**

```
scripts/cargo_lane.sh test -p rs_cam_viz -j 2 --test the_corridor_bounds_the_band_g_corridor
```

If it panics on `measured()`'s `.expect(...)`, the fixture `LONG_AND_THIN`
(Ø1.5, 90 mm stickout) now has its edge force above budget. That is the same
true finding as the `gate_honors_optimizer_feed_down_into_within` repair: some
cuts are no longer rescuable by feed. Re-derive the fixture; do not widen the
arm. The file's own doc block (lines 12-23) says so.

---

## 2. Standing rules — these bind every step

- **Run every cargo command through `scripts/cargo_lane.sh <args>`.** It takes
  a lock, waits for foreign cargo/rustc and for 12 GB free, then runs `nice`.
  Two concurrent builds have crashed this machine.
- **Never run the core full gate locally without asking.** CI owns it. Ask
  before any run over about three minutes.
- `rs_cam_viz` tests need `-j 2`. Its 46 test targets are what took the
  machine down.
- **The working tree and the git INDEX are shared with other sessions.** Stage
  by explicit path, verify the staged set immediately before committing, and
  prefer `git commit -- <pathspec>`. Never stash, reset, checkout or clean the
  whole tree. Read baselines with `git show HEAD:<path>`.
- **Never push. Never open a pull request. Never `git reset --hard`.** Never
  stage `.mcp.json` or `.pi/`.
- Prose in ASD-STE100 Simplified Technical English.
- **No bench rig.** Every constant is math-derived-and-checkable or straight
  from named literature. A fabricated constant is worse than an absent one.
  When a value cannot be sourced, leave it unmodelled AND say which direction
  the error goes.
- A sweep that does not fire is evidence about the sweep, not about the code.

---

## 3. Decisions already taken — do not reopen without new evidence

- **The bending fraction is a flat 0.80, with no flute-count term.** Kops and
  Vo, Annals of the CIRP 39(1):93-96 (1990), measured from compliance. The
  operator ruled on this on 2026-09-17 after the derivation showed the
  published per-flute table's N-trend is an artefact of one thesis's flute
  model, and that its two-flute figure is the arithmetic mean where a rotating
  tool needs the harmonic mean. See `FLUTE_SECTION_MATH.md`.
- **Flat, ball, bull and tapered ball all take the fraction. A V-bit does
  not**, and that gap is NON-conservative. T-4 makes it visible.
- **T-7 is withdrawn** — the helix-wrap premise fails.
- **`MILLING_KC_FACTOR` is inert on every load path** — it cancels between the
  numerator and the anchor. Verified numerically. Do not re-derive it.
- **The "+36 %" predictor over-shoot is withdrawn**, not re-measured. It was
  written against a predictor deleted ten days later.

---

## 4. DONE 2026-09-18: T-4 — a refusal must not read as a measurement

Shipped at `59822cab`. `Result<DeflectionPrediction, DeflectionUnmodeled>`,
nine variants, one clause each, mapped onto `UnmodeledReason`. A V-bit
returns `Ok` with `DeflectionCaveat::FluteReliefUnmodeled` (a floor, not a
bound; `force_headroom` returns `None` for one). The back-off emits
`DeflectionBackoffUnmodeled` / `DeflectionBackoffFigureIsAFloor`. The design
missed one reachable zero (`tip_deflection_mm` returns 0.0 when the load
point sits at or below the tip); `DegenerateCantilever` now guards it. The
two panic arms added to `wanaka_suggest_integration` were argued from the
fixture first, then run locally on 2026-09-18 with T-4, T-18 and T-9 all in
the tree: 3/3 green, neither arm fired. Report: `T4_IMPLEMENTATION.md`.

The section below is the design as it stood before the work, kept as the
record.

`predict_peak_deflection_um` has **nine** refusal exits and every one returns
`predicted_um = 0.0`. A modelled zero is unreachable, so today every `0.0` is
an absence. Four production callers read it. The back-off treats the refusal
as a pass-through and says nothing.

**Full design, with the enum already written out: `T4_REFUSAL.md` section 4.**
Follow it. The decisions inside it are settled:

- Reuse `tool_load::verdict::UnmodeledReason` as the shared VOCABULARY so the
  pre-sim and post-sim abstentions print the same words.
- Do NOT reuse `CriterionStatus` as the container — it carries
  `sample_range`, `population`, `confidence` and `exceeded`, which a
  closed form at one operating point cannot fill.
- Do NOT reuse `GatePopulation` — X-VAC is about an empty evidence set; this
  is about an inapplicable model.
- The shape to copy is `feeds::force::DeflectionCapRefusal` (`force.rs:167`),
  in the same folder. Third time the repository has drawn this.

Work items:

1. Add `DeflectionUnmodeled` and `as_unmodeled_reason`, per T4_REFUSAL §4.
2. **Delete the stale V-bit blanket guard** at `predict.rs:219`. Its comment
   says "the post-sim integrator handles V-bits; the closed-form predictor
   refuses", but since `3b0dc487` the predictor CALLS that integrator, and
   `cutter_constraints.rs:716` already computes a V-bit deflection bound from
   it. **But do not simply let the number through**: the derivation says a
   V-bit's cone is modelled while its flute relief is not, and that error is
   non-conservative. Return the value carrying a `FluteReliefUnmodeled` mark.
   This synthesis is the point — one agent said "delete the guard", another
   said "a V-bit is unmodelled", and both are half right.
3. The two UNDOCUMENTED refusals at `predict.rs:396`, hidden in a
   `.map_or(0.0, …)`: `Material::Custom` refuses there even when
   `kc_n_per_mm2()` returned `Some`, and a zero stickout refuses there. The
   module header under-states its own refusal set.
4. `CutterOpProfile::predictions` (`feeds/profile.rs:206`) has **no reader
   anywhere**, production or test. Propose deleting it rather than migrating
   it through the signature change. Confirm with `rg` first — the graph tools
   under-report callers.
5. Make the back-off ACT on the refusal instead of passing through silently.
   The sharp case: a V-bit or an unvalidated plastic on a roughing operation
   skips the DPP back-off and says nothing.

Sentry: name it for the claim, `<claim>_g_<code>.rs`, and give it a
non-vacuity anchor. Read `crates/rs_cam_core/tests/CLAUDE.md` first.

---

## 5. DONE 2026-09-18: T-18 — a feed lift caps against the travel rate

Shipped at `d47a04d8`. `MachineProfile::commanded_cutting_feed_ceiling_mm_min()`
states the invariant; four sites read it (the fourth, `adaptive_entry.rs`
"Step 7 re-applied", had the mismatch in the other direction and no test
pinned it). Red-first sentry: 750 / 600 / 800 mm/min against a 400 cap on
the parent; 400 after. No preset number moved. Report: `T18_IMPLEMENTATION.md`.

**On the single-guard question (T-15's structure), from the T-18 report:** a
guard that re-checks every ceiling after the LAST lift is the better
structure, and T-18 did not build it. It would sit twice — at the end of
`feeds::calculate` after Step 9c, and at the end of `enforce_invariants`
after pass 9 — because Suggest moves the feed outside the calculator. It
needs every ceiling re-evaluated at the FINAL operating point (cheap for the
machine ceiling, expensive for power: the open `G-SUGGEST-POWERSTALE` row),
and one rule for an up-pushing floor against a down-pushing ceiling that
does not delete a warning an earlier step filed. That is T-15's work. T-18
gives it a helper to call rather than an expression to re-derive.

The section below is the design as it stood before the work.

Full entry in `TECH_DEBT_REGISTER.md`. Three sites cap a feed lift at
`machine.max_feed_mm_min * safety_factor` — the gantry TRAVEL rate — while
Step 7 clamped at `cutting_feed_ceiling_mm_min()`:

- `feeds/mod.rs:2349` (rubbing-floor lift)
- `feeds/mod.rs:2381` (drill envelope clamp)
- `feeds/suggest/adaptive_entry.rs:470`

`adaptive_entry.rs` uses BOTH, 49 lines apart. Step 7's own comment says the
rule: *"the calculator emits CUTTING feeds — clamp at the cutting ceiling, not
the gantry travel rate."*

**Decision taken 2026-09-18, from the code's own two-axis vocabulary
(`feeds/mod.rs` ~1840, "RAW axis" vs "COMMANDED axis"):**

- Step 7 runs BEFORE Step 9 and caps the RAW feed at the ceiling.
- Step 9 multiplies the feed by `safety_factor`. The calculator's output
  invariant is therefore `commanded_feed <= ceiling * safety_factor`.
- The three lift sites run AFTER Step 9, on the COMMANDED axis. Their
  `* safety_factor` is the right shape. Their base quantity is wrong: the
  travel rate instead of the cutting ceiling. Class-2 defect.

So: cap every post-Step-9 lift at `cutting_feed_ceiling_mm_min() *
safety_factor`. Add one helper on `MachineProfile` (name it for the axis,
for example `commanded_cutting_feed_ceiling_mm_min`) with a doc that states
the invariant, and use it at all three sites. On the shipped presets
(travel 4000/5000/5000, all under the 6000 default cap) the ceiling equals
the travel rate, so no shipped number moves. The defect bites a profile with
an explicit `max_cutting_feed_mm_min` below travel, or a gantry faster than
6000 / safety.

`adaptive_entry.rs:421` ("Step 7 re-applied") caps the RESCALED feed at the
RAW-axis ceiling with no factor, and `rescaled` is a post-Step-9 quantity.
That is the same axis mismatch in the other direction. The T-18 agent must
measure whether moving it to the helper changes any fixture, and report
before it changes behaviour there.

Latent today only because the rubbing floor caps chipload at 0.025 mm/tooth on
the shipped presets. That is evidence about the presets.

Same structure as **T-15** (a late step raises a feed the power ladder
clamped). Consider one guard that re-checks every ceiling after the last lift,
rather than three patches plus a fourth for T-15.

---

## 6. DONE 2026-09-18: T-9 — small, taken while the file was open

Shipped at `6a9330dc`. `round_suggestion_value_down` floors the feed and the
plunge in `apply`; the pill fallback takes a per-field quantiser; the
export machine-safety pass receives the travel rate. Two tests that encoded
the old half-step were re-derived (one-sided, one full step), and two
pinned shipped feeds in `arc_fit_disposition_a5` moved with their cause
(881 → 880, 638 → 637). The `explore.rs` doc comments are corrected.
Verified under the tree with T-4 and T-18: all twelve apply-door
integration targets green including Wanaka; CLI, MCP and viz lib green.
Report: `T9_IMPLEMENTATION.md`.

**A lesson for the next task that moves a shipped number:** `--lib` does not
reach a core integration target. Two reds were found by other agents, not by
the allowed runs. Before committing a change that moves shipped feeds,
`rg -l` the door symbols across `crates/rs_cam_core/tests/` and run every
target the list names (ask first for Wanaka).

The section below is the design as it stood before the work.

`feeds/suggest/apply.rs:79` rounds the feed to the NEAREST whole mm/min after
every clamp has bound it. Worst case **+0.5 mm/min**, against ADVISORY
ceilings only; no kinematic maximum is breached. Add a round-DOWN twin of
`round_suggestion_value`, the same shape as `machine::next_rpm_at_or_below`.
Detail in `T9_CEILING.md`.

Also from that survey, cheap and worth doing: `rs_cam_viz/src/io/export.rs`
`machine_safety_pass` passes `max_feed_mm_min: None` into the validator,
which disables the one check that could catch any of this in the emitted
file. Its own doc calls this a plumbing gap. **Decision 2026-09-18:** pass
the TRAVEL rate (`machine.max_feed_mm_min`), not the cutting ceiling. The
validator checks every `F` word, and the export rewrites some rapids as
feeds at travel rate, so the ceiling would false-positive on every rewritten
rapid. The travel rate catches a true kinematic overrun. All four callers
hold the session, so the profile is reachable.

---

## 7. THEN: the limits surface — the original request

**Do not start until the `rs_cam_viz` wave lands.** Design is in `PLAN.md`
(§11 recommends Reading A: depth as a post-sim criterion, not gating
initially), `SURVEY_UI.md` and `REVIEW_DESIGN.md`. The interactive prototype
is `derate_bench.html`.

The important discovery: **`CriterionStatus` and `verdict_badge` already do
most of this.** The prototype was 80 % a reimplementation, and worse on the
chipload floor percent. Extend the existing surface; do not build a parallel
one.

The operator's stated requirements, verbatim in effect:

- Treat every limit as the same kind of thing: **0 to limit**.
- Wording like "limit calculated from X" belongs in a hover `(i)`, not on the
  face, and in standardised technical English.
- It must **trace back to the setting that set it** — machine kinematics, tool
  choice, material.
- *"Treat it as a multimeter, not a verbal explanation."*
- After simulation, show the **distribution and the limits**, with the full
  graphs behind a click-through modal. At the toolpath stage, where those
  metrics do not exist, show predictions.

**Two rulings still needed from the operator before building:**

1. Drop the `≈` confidence mark, or keep it?
2. Reinstate the power bar on the Feeds card? It was removed on a 23.6 % peak
   utilisation figure that is now stale — re-measured after R1: median 17.8 %,
   p90 89.4 %, peak 100 %. The doc's own stated condition for reinstating is
   met.

---

## 8. Still open, lower priority

- **T-15** — pass 9 can raise a feed the power ladder clamped. Reachable by
  hand today. Pairs with T-18.
- **T-14** — a drop-cutter finishing pass measures 42.5 mm of axial
  engagement. The power ladder made it load-bearing.
- **T-2, T-3, T-5** — guard and structure debt, no physics.
- **`rs_cam_viz/src/ui/feeds/explore.rs`** (found by the corridor repair,
  2026-09-18): the `Ceiling::OffScale` doc near line 306 prints a
  9.36 / 0.1176 mm/tooth pair for the STUBBY cut that measures
  2.4315 / 0.12353 (the 0.1176 belongs to 18000 RPM; the cut runs 17000).
  Near line 324 it also says the compliance rises with the cube of
  stickout; the integrator is a stepped cantilever, and at Ø2 mm three
  times the stickout gives 1.44x the compliance. Both are doc comments in
  a production file. Fix them with the T-9 viz change, after the viz wave
  lands. Numbers and derivation: `CORRIDOR_REPAIR.md`.
- **T-10** (gantry thrust) and **T-13** (`F_edge` per mm of depth) are
  BLOCKED on a number that cannot be sourced. Under the no-bench-rig rule they
  stay unmodelled. Do not invent a constant to close them.

---

## 9. The three defect classes this repository keeps rediscovering

Every confirmed defect in this programme is one of these. When reviewing new
code, look for them by name:

1. **A value computed at one state, consumed at another.** (`cut_efficiency`,
   `realised_step_down`, `mrr_mm3_min`.)
2. **A quantity multiplied by a fraction of a DIFFERENT quantity.** (T-11;
   the GUI nomogram's `preview_power_kw`; the optimizer's stepover advice;
   T-17's engagement-vs-bending diameter; T-18's travel-vs-cutting feed.)
3. **An absence rendering as a reading.** (T-4; the CLI collision defect;
   `feeds::calculate` reporting `power_kw = 0.0` for a material with no `Kc`.)

Class 2 is the most common and the hardest to see, because the two quantities
share units and magnitude. Only the name differs.
